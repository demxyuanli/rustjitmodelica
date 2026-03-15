import { dia, shapes } from "@joint/core";
import { getActiveScheme } from "./diagramColorSchemes";

export interface JointPaperHandle {
  zoomIn: (opts?: { duration?: number }) => void;
  zoomOut: (opts?: { duration?: number }) => void;
  fitView: (opts?: { padding?: number; duration?: number }) => void;
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

export function resolveDiagramColors(): {
  bg: string;
  bgElevated: string;
  border: string;
  text: string;
  textMuted: string;
  primary: string;
} {
  const base = resolveThemeColors();
  const scheme = getActiveScheme();
  const primary = scheme.diagramPrimary ?? base.primary;
  return { ...base, primary };
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
}

export function createPaper(opts: CreatePaperOptions): dia.Paper {
  const { el, graph, gridSize = 10, readOnly = false } = opts;
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

  const ro = new ResizeObserver((entries) => {
    for (const entry of entries) {
      const { width: newW, height: newH } = entry.contentRect;
      if (newW > 0 && newH > 0) {
        paper.setDimensions(newW, newH);
      }
    }
  });
  ro.observe(el);

  const origRemove = paper.remove.bind(paper);
  paper.remove = function () {
    ro.disconnect();
    return origRemove();
  };

  setupPanAndZoom(paper, opts.onScale, opts.onTranslate, readOnly);
  return paper;
}

function setupPanAndZoom(
  paper: dia.Paper,
  onScale?: (scale: number) => void,
  onTranslate?: (tx: number, ty: number) => void,
  allowPanFromElement = false
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

  paper.on("blank:pointerdown", (evt: dia.Event) => startPanFromEvt(evt));
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

  svgEl.addEventListener("mousemove", (evt: MouseEvent) => {
    if (!isPanning) return;
    const dx = evt.clientX - panStart.x;
    const dy = evt.clientY - panStart.y;
    paper.translate(translateStart.tx + dx, translateStart.ty + dy);
    onTranslate?.(translateStart.tx + dx, translateStart.ty + dy);
  });

  svgEl.addEventListener("mouseup", () => {
    isPanning = false;
  });

  svgEl.addEventListener("mouseleave", () => {
    isPanning = false;
  });

  svgEl.addEventListener("wheel", (evt: WheelEvent) => {
    evt.preventDefault();
    const delta = evt.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP;
    const currentScale = paper.scale().sx;
    const newScale = Math.max(MIN_SCALE, Math.min(MAX_SCALE, currentScale + delta));
    if (newScale === currentScale) return;

    const localPoint = paper.clientToLocalPoint({ x: evt.clientX, y: evt.clientY });
    paper.scale(newScale, newScale);
    const newLocal = paper.clientToLocalPoint({ x: evt.clientX, y: evt.clientY });
    const t = paper.translate();
    const adjustedTx = t.tx + (newLocal.x - localPoint.x) * newScale;
    const adjustedTy = t.ty + (newLocal.y - localPoint.y) * newScale;
    paper.translate(adjustedTx, adjustedTy);
    onScale?.(newScale);
    onTranslate?.(adjustedTx, adjustedTy);
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
