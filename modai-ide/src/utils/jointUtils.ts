import { dia, shapes } from "@joint/core";
import { getActiveScheme } from "./diagramColorSchemes";

export interface JointPaperHandle {
  zoomIn: (opts?: { duration?: number }) => void;
  zoomOut: (opts?: { duration?: number }) => void;
  fitView: (opts?: { padding?: number; duration?: number }) => void;
  /** Scale 1:1 and reset pan origin. */
  resetZoom100: () => void;
  /** Fit viewport to the union bbox of element ids (diagram mode). */
  zoomToElementIds: (elementIds: string[]) => void;
  getScale: () => number;
  getTranslate: () => { tx: number; ty: number };
  paper: dia.Paper | null;
}

const MIN_SCALE = 0.1;
const MAX_SCALE = 3.0;
const ZOOM_STEP = 0.15;

export function resolveThemeColors(): {
  bg: string;
  bgElevated: string;
  border: string;
  text: string;
  textMuted: string;
  primary: string;
} {
  const probe = document.createElement("div");
  probe.style.cssText =
    "position:absolute;left:-9999px;width:0;height:0;" +
    "color:var(--text);background:var(--bg);border:1px solid var(--border);";
  document.body.appendChild(probe);
  const cs = getComputedStyle(probe);
  const bg = cs.backgroundColor || "#f8fafc";
  const border = cs.borderColor || "#cbd5e1";
  const text = cs.color || "#1e293b";
  document.body.removeChild(probe);

  const probe2 = document.createElement("div");
  probe2.style.cssText =
    "position:absolute;left:-9999px;width:0;height:0;" +
    "background:var(--bg-elevated);color:var(--text-muted);border:1px solid var(--primary);";
  document.body.appendChild(probe2);
  const cs2 = getComputedStyle(probe2);
  const bgElevated = cs2.backgroundColor || "#ffffff";
  const textMuted = cs2.color || "#64748b";
  const primary = cs2.borderColor || "#3b82f6";
  document.body.removeChild(probe2);

  return { bg, bgElevated, border, text, textMuted, primary };
}

type ResolvedDiagramColors = {
  bg: string;
  bgElevated: string;
  border: string;
  text: string;
  textMuted: string;
  primary: string;
};

let diagramColorsCache: { schemeId: string; colors: ResolvedDiagramColors } | null = null;

export function resolveDiagramColors(): ResolvedDiagramColors {
  const scheme = getActiveScheme();
  const schemeId = scheme.id;
  if (diagramColorsCache?.schemeId === schemeId) {
    return diagramColorsCache.colors;
  }
  const base = resolveThemeColors();
  const primary = scheme.diagramPrimary ?? base.primary;
  const colors = { ...base, primary };
  diagramColorsCache = { schemeId, colors };
  return colors;
}

export function invalidateDiagramColorsCache(): void {
  diagramColorsCache = null;
}

export function getConnectorColor(kind?: string): string {
  if (!kind) return "#888";
  const scheme = getActiveScheme();
  return scheme.connectorColors[kind] ?? "#888";
}

export interface CreatePaperOptions {
  el: HTMLElement;
  graph: dia.Graph;
  gridSize?: number;
  readOnly?: boolean;
  onScale?: (scale: number) => void;
  onTranslate?: (tx: number, ty: number) => void;
  /** When false, blank pointer-drag does not pan (used for shift+marquee selection). */
  allowBlankPan?: (evt: dia.Event) => boolean;
}

export function createPaper(opts: CreatePaperOptions): dia.Paper {
  const { el, graph, gridSize = 10, readOnly = false, allowBlankPan } = opts;
  const colors = resolveDiagramColors();

  const rect = el.getBoundingClientRect();
  const w = Math.max(rect.width, 200);
  const h = Math.max(rect.height, 200);

  const paper = new dia.Paper({
    el,
    model: graph,
    width: w,
    height: h,
    gridSize,
    drawGrid: { name: "dot", args: { color: colors.border } },
    background: { color: colors.bg },
    async: false,
    cellViewNamespace: shapes,
    interactive: readOnly
      ? false
      : {
          elementMove: true,
          addLinkFromMagnet: true,
          labelMove: false,
        },
    snapLinks: { radius: 15 },
    defaultRouter: { name: "manhattan", args: { step: gridSize } },
    defaultConnector: { name: "rounded", args: { radius: 4 } },
    defaultLink: () =>
      new shapes.standard.Link({
        attrs: {
          line: {
            stroke: colors.textMuted,
            strokeWidth: 1.5,
            targetMarker: {
              type: "path",
              d: "M 10 -5 0 0 10 5 Z",
              fill: colors.textMuted,
            },
          },
        },
      }),
    validateConnection: (
      _cellViewS,
      magnetS,
      _cellViewT,
      magnetT,
      _end,
      _linkView
    ) => {
      if (!magnetS || !magnetT) return false;
      if (magnetS === magnetT) return false;
      return true;
    },
    linkPinning: false,
  });

  const RESIZE_EPS = 0.75;
  let resizeRaf: number | null = null;
  let pendingW = 0;
  let pendingH = 0;
  const applyPendingResize = () => {
    resizeRaf = null;
    if (pendingW <= 0 || pendingH <= 0) return;
    try {
      const cur = paper.getComputedSize();
      if (
        Math.abs(cur.width - pendingW) < RESIZE_EPS &&
        Math.abs(cur.height - pendingH) < RESIZE_EPS
      ) {
        return;
      }
      paper.setDimensions(pendingW, pendingH);
    } catch {
      // paper may be tearing down
    }
  };
  const ro = new ResizeObserver((entries) => {
    for (const entry of entries) {
      const { width: newW, height: newH } = entry.contentRect;
      if (newW > 0 && newH > 0) {
        pendingW = newW;
        pendingH = newH;
        if (resizeRaf == null) {
          resizeRaf = requestAnimationFrame(applyPendingResize);
        }
      }
    }
  });
  ro.observe(el);

  const origRemove = paper.remove.bind(paper);
  paper.remove = function () {
    ro.disconnect();
    if (resizeRaf != null) {
      cancelAnimationFrame(resizeRaf);
      resizeRaf = null;
    }
    return origRemove();
  };

  setupPanAndZoom(paper, opts.onScale, opts.onTranslate, readOnly, allowBlankPan);
  return paper;
}

function setupPanAndZoom(
  paper: dia.Paper,
  onScale?: (scale: number) => void,
  onTranslate?: (tx: number, ty: number) => void,
  allowPanFromElement = false,
  allowBlankPan?: (evt: dia.Event) => boolean,
) {
  let isPanning = false;
  let panStart = { x: 0, y: 0 };
  let translateStart = { tx: 0, ty: 0 };

  const startPanFromEvt = (evt: { clientX?: number; clientY?: number }) => {
    isPanning = true;
    panStart = { x: evt.clientX ?? 0, y: evt.clientY ?? 0 };
    const t = paper.translate();
    translateStart = { tx: t.tx, ty: t.ty };
  };

  paper.on("blank:pointerdown", (evt: dia.Event) => {
    if (allowBlankPan && !allowBlankPan(evt)) return;
    startPanFromEvt(evt);
  });
  if (allowPanFromElement) {
    paper.on("element:pointerdown", (_elementView: dia.ElementView, evt: dia.Event) => {
      startPanFromEvt(evt);
    });
  }

  paper.on("blank:pointerup", () => { isPanning = false; });
  if (allowPanFromElement) {
    paper.on("element:pointerup", () => { isPanning = false; });
  }

  const svgEl = paper.el;

  let translateRaf: number | null = null;
  let latestTx = 0;
  let latestTy = 0;
  svgEl.addEventListener("mousemove", (evt: MouseEvent) => {
    if (!isPanning) return;
    const dx = evt.clientX - panStart.x;
    const dy = evt.clientY - panStart.y;
    latestTx = translateStart.tx + dx;
    latestTy = translateStart.ty + dy;
    // Batch translate to RAF instead of calling paper.translate() per pixel
    if (translateRaf == null) {
      translateRaf = requestAnimationFrame(() => {
        translateRaf = null;
        paper.translate(latestTx, latestTy);
        onTranslate?.(latestTx, latestTy);
      });
    }
  });

  svgEl.addEventListener("mouseup", () => {
    isPanning = false;
  });

  svgEl.addEventListener("mouseleave", () => {
    isPanning = false;
  });

  const clearLocalPan = () => {
    isPanning = false;
    if (translateRaf != null) {
      cancelAnimationFrame(translateRaf);
      translateRaf = null;
    }
  };

  const releaseJointDocumentCapture = () => {
    clearLocalPan();
    try {
      paper.undelegateDocumentEvents();
    } catch {
      // ignore if paper is tearing down
    }
  };

  window.addEventListener("mouseup", clearLocalPan, true);
  window.addEventListener("pointerup", clearLocalPan, true);
  window.addEventListener("pointercancel", releaseJointDocumentCapture, true);
  window.addEventListener("blur", releaseJointDocumentCapture);

  const onVisibilityChange = () => {
    if (document.visibilityState === "hidden") {
      releaseJointDocumentCapture();
    }
  };
  document.addEventListener("visibilitychange", onVisibilityChange);

  const origRemovePan = paper.remove.bind(paper);
  paper.remove = function () {
    window.removeEventListener("mouseup", clearLocalPan, true);
    window.removeEventListener("pointerup", clearLocalPan, true);
    window.removeEventListener("pointercancel", releaseJointDocumentCapture, true);
    window.removeEventListener("blur", releaseJointDocumentCapture);
    document.removeEventListener("visibilitychange", onVisibilityChange);
    return origRemovePan();
  };

  let zoomRaf: number | null = null;
  let pendingScale = 0;
  let pendingCx = 0;
  let pendingCy = 0;
  svgEl.addEventListener("wheel", (evt: WheelEvent) => {
    evt.preventDefault();
    const delta = evt.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP;
    const currentScale = pendingScale || paper.scale().sx;
    const newScale = Math.max(MIN_SCALE, Math.min(MAX_SCALE, currentScale + delta));
    if (newScale === currentScale) return;
    pendingScale = newScale;
    pendingCx = evt.clientX;
    pendingCy = evt.clientY;
    if (zoomRaf == null) {
      zoomRaf = requestAnimationFrame(() => {
        zoomRaf = null;
        const ns = pendingScale;
        pendingScale = 0;
        const localPoint = paper.clientToLocalPoint({ x: pendingCx, y: pendingCy });
        paper.scale(ns, ns);
        const newLocal = paper.clientToLocalPoint({ x: pendingCx, y: pendingCy });
        const t = paper.translate();
        const adjustedTx = t.tx + (newLocal.x - localPoint.x) * ns;
        const adjustedTy = t.ty + (newLocal.y - localPoint.y) * ns;
        paper.translate(adjustedTx, adjustedTy);
        onScale?.(ns);
        onTranslate?.(adjustedTx, adjustedTy);
      });
    }
  }, { passive: false });
}

export function createPaperHandle(paper: dia.Paper | null): JointPaperHandle {
  return {
    paper,
    zoomIn(_opts) {
      if (!paper) return;
      const s = paper.scale().sx;
      const ns = Math.min(MAX_SCALE, s + ZOOM_STEP);
      paper.scale(ns, ns);
    },
    zoomOut(_opts) {
      if (!paper) return;
      const s = paper.scale().sx;
      const ns = Math.max(MIN_SCALE, s - ZOOM_STEP);
      paper.scale(ns, ns);
    },
    fitView(_opts) {
      if (!paper) return;
      try {
        paper.transformToFitContent({
          padding: 30,
          maxScale: 1.5,
          minScale: MIN_SCALE,
        });
      } catch (_) {
        // no content yet
      }
    },
    resetZoom100() {
      if (!paper) return;
      paper.scale(1, 1);
      paper.translate(0, 0);
    },
    zoomToElementIds(ids: string[]) {
      if (!paper) return;
      const gr = paper.model as dia.Graph;
      const elems = ids
        .map((id) => gr.getCell(id))
        .filter((c): c is dia.Element => Boolean(c && c.isElement()));
      if (!elems.length) return;
      const bbox = gr.getCellsBBox(elems);
      if (!bbox || bbox.width <= 0 || bbox.height <= 0) return;
      try {
        paper.scaleContentToFit({
          padding: 24,
          maxScale: 2,
          minScale: MIN_SCALE,
          fittingBBox: bbox,
        });
      } catch {
        /* ignore */
      }
    },
    getScale() {
      if (!paper) return 1;
      return paper.scale().sx;
    },
    getTranslate() {
      if (!paper) return { tx: 0, ty: 0 };
      const t = paper.translate();
      return { tx: t.tx, ty: t.ty };
    },
  };
}
