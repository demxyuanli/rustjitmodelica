import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { dia, shapes, linkTools } from "@joint/core";
import {
  createElement,
  createLink,
  determineNodeShape,
  type NodeShape,
} from "../components/diagram/jointShapes";
import {
  createPaper,
  createPaperHandle,
  resolveDiagramColors,
  type JointPaperHandle,
} from "../utils/jointUtils";
import { useDiagramScheme } from "../contexts/DiagramSchemeContext";
import { JointMiniMap } from "../components/diagram/JointMiniMap";
import { ContextMenu, type ContextMenuItem } from "../components/ContextMenu";
import { DEFAULT_HANDLE_ID } from "./docSync";
import type { DiagramLink, DiagramNode, DiagramNodeData } from "./types";
import type { StructureGraphSession, StructureSnapMode } from "./session";
import { snapPositionWithAlignmentGuides, type BoxLayout } from "./alignmentGuides";
import { ALIGN_GUIDE_COLOR, STRUCTURE_GRID_DEFAULT } from "./layoutConstants";

const CELL_SIG_KEY = "seCellSig";

export interface JointStructureEditorProps {
  session: StructureGraphSession;
  readOnly: boolean;
  onDoubleClick?: (typeName: string, libraryId?: string) => void;
  onAltDoubleClick?: (typeName: string, libraryId?: string) => void;
  onPaperReady?: (handle: JointPaperHandle | null) => void;
  showMiniMap?: boolean;
  snapToGrid?: boolean;
  /** When set, overrides legacy snapToGrid (none vs grid). */
  snapMode?: StructureSnapMode;
  structureGridSize?: number;
  onPointerPaperLocal?: (pt: { x: number; y: number } | null) => void;
  canConnect?: (
    source: string,
    sourcePort: string,
    target: string,
    targetPort: string,
  ) => boolean;
  onDiagramSelection?: () => void;
}

function shapeForData(data: DiagramNodeData): NodeShape {
  return determineNodeShape({
    isInput: data.isInput,
    isOutput: data.isOutput,
    isSourceNode: data.isSourceNode,
    isSinkNode: data.isSinkNode,
  });
}

function rq(n: number) {
  const f = 1e4;
  return Math.round(n * f) / f;
}

function vertKey(v?: { x: number; y: number }[]) {
  if (!v?.length) return "";
  return JSON.stringify(v.map((p) => ({ x: rq(p.x), y: rq(p.y) })));
}

/** Stable signature for remount vs in-place update; avoid JSON.stringify on hot path. */
function cellSignature(node: DiagramNode): string {
  const d = node.data;
  const ph = d?.portHandles?.join("\x1e") ?? "";
  const pr =
    d?.params
      ?.map((p) => `${p.name ?? ""}\x1f${p.value ?? ""}`)
      .join("\x1e") ?? "";
  const parts = [
    node.id,
    rq(node.position.x),
    rq(node.position.y),
    d?.typeName ?? "",
    d?.libraryId ?? "",
    ph,
    d?.connectorKind ?? "",
    d?.isInput ? "1" : "0",
    d?.isOutput ? "1" : "0",
    d?.isSourceNode ? "1" : "0",
    d?.isSinkNode ? "1" : "0",
    String(d?.rotation ?? ""),
    pr,
    d?.hasError ? "1" : "0",
    d?.errorMessage ?? "",
    d?.replaceable ? "1" : "0",
    d?.constrainedbyType ?? "",
    d?.condition ?? "",
    d?.visible === false ? "0" : "1",
  ];
  return parts.join("\x1d");
}

type Colors = ReturnType<typeof resolveDiagramColors>;

function paintBody(
  el: dia.Element,
  elId: string,
  hover: string | null,
  selected: Set<string>,
  colors: Colors,
  byId: Map<string, DiagramNode>,
  sigCache: Map<string, string>,
) {
  const sel = selected.has(elId);
  const hov = elId === hover;
  const node = byId.get(elId);
  const err = Boolean(node?.data?.hasError);
  const vis = node?.data?.visible !== false;
  const sig = `${sel ? 1 : 0}|${hov ? 1 : 0}|${err ? 1 : 0}|${vis ? 1 : 0}`;
  if (sigCache.get(elId) === sig) return;
  sigCache.set(elId, sig);
  const opacity = vis ? 1 : 0.38;
  el.attr("body/opacity", opacity);
  el.attr("label/opacity", opacity);
  if (sel) {
    el.attr("body/stroke", colors.primary);
    el.attr("body/strokeWidth", 2);
  } else if (hov) {
    el.attr("body/stroke", colors.primary);
    el.attr("body/strokeWidth", 1);
  } else {
    el.attr("body/stroke", err ? "#ef4444" : colors.border);
    el.attr("body/strokeWidth", 1);
  }
}

function mountNode(g: dia.Graph, node: DiagramNode) {
  const shape = shapeForData(node.data);
  const ports = node.data.portHandles?.length ? node.data.portHandles : [DEFAULT_HANDLE_ID];
  const paramStr = node.data.params
    ?.filter((x) => x.value)
    .map((x) => (x.name ? `${x.name}=${x.value}` : x.value))
    .join(", ");
  const el = createElement({
    id: node.id,
    shape,
    position: node.position,
    label: node.id,
    sublabel: node.data.typeName,
    paramStr,
    ports,
    connectorKind: node.data.connectorKind,
    hasError: node.data.hasError,
    errorMessage: node.data.errorMessage,
  });
  el.set(CELL_SIG_KEY, cellSignature(node));
  g.addCell(el);
}

function linkRouterName(link: dia.Link): string {
  const r = link.prop("router") as { name?: string } | string | undefined;
  if (r && typeof r === "object" && r.name) return r.name;
  return "manhattan";
}

function syncLinkGeometry(jl: dia.Link, link: DiagramLink, gridStep: number) {
  if (link.vertices?.length && vertKey(jl.vertices()) !== vertKey(link.vertices)) {
    jl.vertices(link.vertices);
  }
  const want = link.router ?? "manhattan";
  if (linkRouterName(jl) !== want) {
    jl.router({ name: want, args: { step: gridStep } });
  }
}

function reconcileGraph(g: dia.Graph, nodes: DiagramNode[], links: DiagramLink[], gridStep: number) {
  const perfOk = typeof performance !== "undefined" && typeof performance.mark === "function";
  if (perfOk) {
    try {
      performance.mark("modai-reconcile-start");
    } catch {
      /* ignore */
    }
  }
  g.startBatch("modai-reconcile");
  try {
  const wantN = new Set(nodes.map((n) => n.id));
  const wantL = new Set(links.map((l) => l.id));
  g.getElements().forEach((el) => {
    if (!wantN.has(el.id as string)) el.remove();
  });
  g.getLinks().forEach((l) => {
    if (!wantL.has(l.id as string)) l.remove();
  });

  for (const node of nodes) {
    const cell = g.getCell(node.id);
    const sig = cellSignature(node);
    if (cell?.isElement()) {
      const el = cell as dia.Element;
      const pos = el.position();
      if (Math.abs(pos.x - node.position.x) > 1 || Math.abs(pos.y - node.position.y) > 1) {
        el.position(node.position.x, node.position.y);
      }
      const prev = el.get(CELL_SIG_KEY) as string | undefined;
      if (prev != null && prev !== sig) {
        el.remove();
        mountNode(g, node);
      } else {
        el.set(CELL_SIG_KEY, sig);
      }
    } else {
      mountNode(g, node);
    }
  }

  for (const link of links) {
    const cell = g.getCell(link.id);
    if (!cell?.isLink()) {
      g.addCell(
        createLink(
          {
            id: link.id,
            source: link.source,
            sourcePort: link.sourcePort,
            target: link.target,
            targetPort: link.targetPort,
            vertices: link.vertices,
            routerName: link.router,
            gridStep,
          },
          g,
        ),
      );
    } else {
      syncLinkGeometry(cell as dia.Link, link, gridStep);
    }
  }
  } finally {
    g.stopBatch("modai-reconcile");
    if (perfOk) {
      try {
        performance.mark("modai-reconcile-end");
        performance.measure("modai-reconcile", "modai-reconcile-start", "modai-reconcile-end");
        const e = performance.getEntriesByName("modai-reconcile").pop();
        // ~32ms ~= one missed frame at 30Hz; structural sync can legitimately exceed 16ms.
        if (
          import.meta.env.DEV &&
          e &&
          "duration" in e &&
          e.duration > 32
        ) {
          console.warn(`[modai] reconcileGraph slow: ${e.duration.toFixed(1)}ms`);
        }
      } catch {
        /* ignore */
      }
      try {
        performance.clearMarks("modai-reconcile-start");
        performance.clearMarks("modai-reconcile-end");
        performance.clearMeasures("modai-reconcile");
      } catch {
        /* ignore */
      }
    }
  }
}

export function JointStructureEditor({
  session,
  readOnly,
  onDoubleClick,
  onAltDoubleClick,
  onPaperReady,
  showMiniMap = true,
  snapToGrid = true,
  snapMode: snapModeProp,
  structureGridSize = STRUCTURE_GRID_DEFAULT,
  onPointerPaperLocal,
  canConnect,
  onDiagramSelection,
}: JointStructureEditorProps) {
  const snapMode: StructureSnapMode = snapModeProp ?? (snapToGrid ? "grid" : "none");
  const rootRef = useRef<HTMLDivElement>(null);
  const graphRef = useRef<dia.Graph | null>(null);
  const paperRef = useRef<dia.Paper | null>(null);
  const gridStepRef = useRef(structureGridSize);
  gridStepRef.current = structureGridSize;
  const sessionRef = useRef(session);
  sessionRef.current = session;
  const onDblRef = useRef(onDoubleClick);
  onDblRef.current = onDoubleClick;
  const onAltDblRef = useRef(onAltDoubleClick);
  onAltDblRef.current = onAltDoubleClick;
  const canConnRef = useRef(canConnect);
  canConnRef.current = canConnect;
  const onSelRef = useRef(onDiagramSelection);
  onSelRef.current = onDiagramSelection;
  const onPtrLocalRef = useRef(onPointerPaperLocal);
  onPtrLocalRef.current = onPointerPaperLocal;

  const [ctxMenu, setCtxMenu] = useState<{ visible: boolean; x: number; y: number; items: ContextMenuItem[] }>({
    visible: false,
    x: 0,
    y: 0,
    items: [],
  });

  const revision = useSyncExternalStore(
    session.subscribe,
    () => session.getRevision(),
    () => session.getRevision(),
  );

  const selectedRef = useRef<Set<string>>(new Set());
  const hoverRef = useRef<string | null>(null);
  const lastHoverPaintRef = useRef<string | null>(null);
  const strokeSigRef = useRef<Map<string, string>>(new Map());
  const hoverRafRef = useRef<number | null>(null);
  const paintAllRef = useRef<() => void>(() => {});
  const scheduleHoverRef = useRef<() => void>(() => {});

  const { schemeId } = useDiagramScheme();

  const bundle = useMemo(() => {
    void revision;
    return session.getNodesLinksForCanvas(onDblRef.current);
  }, [session, revision, onDoubleClick]);

  const bundleRef = useRef(bundle);
  bundleRef.current = bundle;

  const applyingSessionGraphRef = useRef(false);
  /** `docContentVersion|grid` — only full reconcile when document content or grid step for routing changes. */
  const lastReconciledKeyRef = useRef<string | null>(null);

  const errKey = useMemo(
    () => bundle.nodes.map((n) => `${n.id}:${n.data?.hasError ? 1 : 0}`).join("|"),
    [bundle.nodes],
  );
  const errKeyRef = useRef(errKey);
  errKeyRef.current = errKey;
  const lastErrKeyRef = useRef("");

  paintAllRef.current = () => {
    const g = graphRef.current;
    if (!g) return;
    selectedRef.current = new Set(sessionRef.current.getStructureSelectionIds());
    if (lastErrKeyRef.current !== errKeyRef.current) {
      lastErrKeyRef.current = errKeyRef.current;
      strokeSigRef.current.clear();
    }
    const colors = resolveDiagramColors();
    const byId = new Map(bundleRef.current.nodes.map((n) => [n.id, n]));
    const sel = selectedRef.current;
    const hv = hoverRef.current;
    g.startBatch("se-stroke-all");
    try {
      g.getElements().forEach((el) => {
        paintBody(el as dia.Element, el.id as string, hv, sel, colors, byId, strokeSigRef.current);
      });
    } finally {
      g.stopBatch("se-stroke-all");
    }
    lastHoverPaintRef.current = hv;
  };

  scheduleHoverRef.current = () => {
    if (hoverRafRef.current != null) return;
    hoverRafRef.current = requestAnimationFrame(() => {
      hoverRafRef.current = null;
      const g = graphRef.current;
      if (!g) return;
      selectedRef.current = new Set(sessionRef.current.getStructureSelectionIds());
      const next = hoverRef.current;
      const prev = lastHoverPaintRef.current;
      if (next === prev) return;
      const colors = resolveDiagramColors();
      const byId = new Map(bundleRef.current.nodes.map((n) => [n.id, n]));
      const sel = selectedRef.current;
      g.startBatch("se-stroke-hover");
      try {
        const one = (id: string | null) => {
          if (!id) return;
          const c = g.getCell(id);
          if (c?.isElement()) {
            paintBody(c as dia.Element, id, next, sel, colors, byId, strokeSigRef.current);
          }
        };
        one(prev);
        one(next);
        for (const id of sel) {
          if (id !== prev && id !== next) one(id);
        }
      } finally {
        g.stopBatch("se-stroke-hover");
      }
      lastHoverPaintRef.current = next;
    });
  };

  useEffect(() => {
    selectedRef.current = new Set(session.getStructureSelectionIds());
  }, [session, revision]);

  useEffect(() => {
    session.setStructureSnapOptions({ mode: snapMode, gridSize: structureGridSize });
  }, [session, snapMode, structureGridSize]);

  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    let alive = true;
    let readyRaf: number | null = null;
    let guideMoveRaf: number | null = null;
    let marqueeSuppressBlank = false;

    const clearGuideSvg = () => {
      paperRef.current?.el.querySelector(".modai-structure-guides")?.remove();
    };

    const renderGuides = (paper: dia.Paper, guides: { x1: number; y1: number; x2: number; y2: number }[]) => {
      clearGuideSvg();
      if (!guides.length) return;
      const svg = paper.el.querySelector("svg");
      if (!svg) return;
      const ns = "http://www.w3.org/2000/svg";
      const gg = document.createElementNS(ns, "g");
      gg.setAttribute("class", "modai-structure-guides");
      gg.setAttribute("pointer-events", "none");
      for (const seg of guides) {
        const line = document.createElementNS(ns, "line");
        line.setAttribute("x1", String(seg.x1));
        line.setAttribute("y1", String(seg.y1));
        line.setAttribute("x2", String(seg.x2));
        line.setAttribute("y2", String(seg.y2));
        line.setAttribute("stroke", ALIGN_GUIDE_COLOR);
        line.setAttribute("stroke-width", "1");
        gg.appendChild(line);
      }
      svg.appendChild(gg);
    };

    const clearAllLinkTools = (paper: dia.Paper, gr: dia.Graph) => {
      gr.getLinks().forEach((l) => {
        paper.findViewByModel(l)?.removeTools();
      });
    };

    const el = document.createElement("div");
    el.style.width = "100%";
    el.style.height = "100%";
    if (root.firstChild) root.insertBefore(el, root.firstChild);
    else root.appendChild(el);

    const graph = new dia.Graph({}, { cellNamespace: shapes });
    const paper = createPaper({
      el,
      graph,
      gridSize: snapMode === "none" ? 1 : gridStepRef.current,
      readOnly,
      allowBlankPan: (evt) => {
        const oe = (evt as { originalEvent?: MouseEvent }).originalEvent;
        return !oe?.shiftKey;
      },
    });
    graphRef.current = graph;
    paperRef.current = paper;

    const init = sessionRef.current.getNodesLinksForCanvas(onDblRef.current);
    applyingSessionGraphRef.current = true;
    paper.freeze();
    try {
      reconcileGraph(graph, init.nodes, init.links, gridStepRef.current);
      lastReconciledKeyRef.current = `${sessionRef.current.getDocumentContentVersion()}|${gridStepRef.current}`;
      paintAllRef.current();
    } finally {
      paper.unfreeze();
      setTimeout(() => {
        applyingSessionGraphRef.current = false;
      }, 0);
    }

    const clearMarqueeBox = () => {
      el.querySelector(".modai-structure-marquee")?.remove();
    };

    paper.on("element:pointerclick", (ev: dia.ElementView, evt: dia.Event) => {
      clearAllLinkTools(paper, graph);
      const id = ev.model.id as string;
      const oe = (evt as { originalEvent?: MouseEvent }).originalEvent;
      sessionRef.current.setStructurePointerSelection(id, Boolean(oe?.shiftKey));
      selectedRef.current = new Set(sessionRef.current.getStructureSelectionIds());
      onSelRef.current?.();
    });

    paper.on("link:pointerclick", (lv: dia.LinkView, evt: dia.Event) => {
      const id = lv.model.id as string;
      const oe = (evt as { originalEvent?: MouseEvent }).originalEvent;
      sessionRef.current.setStructurePointerSelection(id, Boolean(oe?.shiftKey));
      selectedRef.current = new Set(sessionRef.current.getStructureSelectionIds());
      onSelRef.current?.();
      if (!readOnly && lv.model?.isLink()) {
        lv.addTools(
          new dia.ToolsView({
            tools: [new linkTools.Vertices(), new linkTools.Segments()],
          }),
        );
      }
    });

    paper.on("blank:pointerclick", () => {
      if (marqueeSuppressBlank) return;
      clearAllLinkTools(paper, graph);
      sessionRef.current.setStructurePointerSelection(null, false);
      selectedRef.current = new Set();
      onSelRef.current?.();
    });

    paper.on("blank:pointerdown", (evt: dia.Event) => {
      const oe = (evt as { originalEvent?: MouseEvent }).originalEvent;
      if (readOnly || !oe?.shiftKey) return;
      const box = document.createElement("div");
      box.className = "modai-structure-marquee";
      box.style.cssText = `position:absolute;border:1px dashed ${ALIGN_GUIDE_COLOR};pointer-events:none;z-index:20;background:rgba(239,68,68,0.08);`;
      const setBoxClient = (x1: number, y1: number, x2: number, y2: number) => {
        const elBox = el.getBoundingClientRect();
        const left = Math.min(x1, x2) - elBox.left;
        const top = Math.min(y1, y2) - elBox.top;
        box.style.left = `${left}px`;
        box.style.top = `${top}px`;
        box.style.width = `${Math.abs(x2 - x1)}px`;
        box.style.height = `${Math.abs(y2 - y1)}px`;
      };
      el.appendChild(box);
      setBoxClient(oe.clientX, oe.clientY, oe.clientX, oe.clientY);
      const onMove = (e: MouseEvent) => {
        setBoxClient(oe.clientX, oe.clientY, e.clientX, e.clientY);
      };
      const onUp = (e: MouseEvent) => {
        window.removeEventListener("mousemove", onMove);
        window.removeEventListener("mouseup", onUp);
        clearMarqueeBox();
        const selRect = paper.clientToLocalRect(
          Math.min(oe.clientX, e.clientX),
          Math.min(oe.clientY, e.clientY),
          Math.abs(e.clientX - oe.clientX),
          Math.abs(e.clientY - oe.clientY),
        );
        if (selRect.width > 3 && selRect.height > 3) {
          const inside = graph.findModelsInArea(selRect);
          const ids = inside.filter((c) => c.isElement()).map((c) => c.id as string);
          sessionRef.current.setStructureMultiSelection(ids);
          selectedRef.current = new Set(sessionRef.current.getStructureSelectionIds());
          onSelRef.current?.();
        }
        marqueeSuppressBlank = true;
        queueMicrotask(() => {
          marqueeSuppressBlank = false;
        });
      };
      window.addEventListener("mousemove", onMove);
      window.addEventListener("mouseup", onUp);
    });

    paper.on("element:mouseenter", (ev: dia.ElementView) => {
      hoverRef.current = ev.model.id as string;
      scheduleHoverRef.current();
    });
    paper.on("element:mouseleave", () => {
      hoverRef.current = null;
      scheduleHoverRef.current();
    });

    paper.on("element:pointermove", (view: dia.ElementView) => {
      if (readOnly || snapMode !== "gridAndGuide") return;
      const m = view.model as dia.Element;
      const bbox = m.getBBox();
      const pos = m.position();
      const others: BoxLayout[] = graph
        .getElements()
        .map((cell) => {
          const ee = cell as dia.Element;
          if (ee.id === m.id) return null;
          const bb = ee.getBBox();
          return { id: ee.id as string, x: bb.x, y: bb.y, width: bb.width, height: bb.height };
        })
        .filter((v): v is BoxLayout => v != null);
      const snap = snapPositionWithAlignmentGuides(m.id as string, pos.x, pos.y, bbox.width, bbox.height, others);
      if (snap.x !== pos.x || snap.y !== pos.y) m.position(snap.x, snap.y);
      if (guideMoveRaf != null) cancelAnimationFrame(guideMoveRaf);
      guideMoveRaf = requestAnimationFrame(() => {
        guideMoveRaf = null;
        renderGuides(paper, snap.guides);
      });
    });

    paper.on("element:pointerup", (ev: dia.ElementView) => {
      clearGuideSvg();
      if (guideMoveRaf != null) {
        cancelAnimationFrame(guideMoveRaf);
        guideMoveRaf = null;
      }
      if (applyingSessionGraphRef.current) return;
      const m = ev.model as dia.Element;
      const p = m.position();
      sessionRef.current.applyNodePosition(m.id as string, { x: p.x, y: p.y });
    });

    const onPaperMouseMove = (e: MouseEvent) => {
      const lp = paper.clientToLocalPoint({ x: e.clientX, y: e.clientY });
      onPtrLocalRef.current?.({ x: lp.x, y: lp.y });
    };
    const onPaperMouseLeave = () => onPtrLocalRef.current?.(null);
    paper.el.addEventListener("mousemove", onPaperMouseMove);
    paper.el.addEventListener("mouseleave", onPaperMouseLeave);

    paper.on("link:connect", (lv: dia.LinkView) => {
      const m = lv.model;
      const s = m.source();
      const t = m.target();
      if (s.id && t.id) {
        const sp = (s.port as string) || DEFAULT_HANDLE_ID;
        const tp = (t.port as string) || DEFAULT_HANDLE_ID;
        if (canConnRef.current?.(s.id as string, sp, t.id as string, tp) ?? true) {
          sessionRef.current.applyConnect(s.id as string, sp, t.id as string, tp);
        }
        m.remove();
      }
    });

    paper.on("link:pointerup", (lv: dia.LinkView) => {
      if (applyingSessionGraphRef.current) return;
      const m = lv.model as dia.Link;
      const v = m.vertices();
      sessionRef.current.applyLinkVerticesIfChanged(
        m.id as string,
        v?.length ? v.map((p) => ({ x: p.x, y: p.y })) : [],
      );
    });

    paper.on("element:pointerdblclick", (ev: dia.ElementView, evt: dia.Event) => {
      const id = ev.model.id as string;
      const node = sessionRef.current.getNodesLinksForCanvas(onDblRef.current).nodes.find((n) => n.id === id);
      if (!node) return;
      const oe = (evt as { originalEvent?: MouseEvent }).originalEvent;
      if (oe?.altKey && onAltDblRef.current) {
        onAltDblRef.current(node.data.typeName, node.data.libraryId as string | undefined);
      } else {
        node.data.onDoubleClick?.(node.data.typeName, node.data.libraryId as string | undefined);
      }
    });

    const handle = createPaperHandle(paper);

    const armReady = () => {
      if (readyRaf != null) cancelAnimationFrame(readyRaf);
      readyRaf = requestAnimationFrame(() => {
        readyRaf = null;
        if (alive) onPaperReady?.(handle);
      });
    };

    if (graph.getElements().length > 0) {
      try {
        paper.transformToFitContent({ padding: 30, maxScale: 1.5, minScale: 0.1 });
      } catch {
        /* ignore */
      }
    }
    armReady();

    return () => {
      alive = false;
      if (readyRaf != null) cancelAnimationFrame(readyRaf);
      onPaperReady?.(null);
      if (hoverRafRef.current != null) cancelAnimationFrame(hoverRafRef.current);
      hoverRafRef.current = null;
      if (guideMoveRaf != null) cancelAnimationFrame(guideMoveRaf);
      guideMoveRaf = null;
      clearGuideSvg();
      clearMarqueeBox();
      paper.el.removeEventListener("mousemove", onPaperMouseMove);
      paper.el.removeEventListener("mouseleave", onPaperMouseLeave);
      paper.remove();
      graphRef.current = null;
      paperRef.current = null;
    };
  }, [readOnly, snapMode]);

  useEffect(() => {
    const p = paperRef.current;
    if (!p) return;
    const o = p.options as { interactive?: unknown; gridSize?: number };
    o.interactive = readOnly ? false : { elementMove: true, addLinkFromMagnet: true, labelMove: false };
    o.gridSize = snapMode === "none" ? 1 : gridStepRef.current;
  }, [readOnly, snapMode, structureGridSize]);

  useEffect(() => {
    const gr = graphRef.current;
    const p = paperRef.current;
    if (!gr) return;
    applyingSessionGraphRef.current = true;
    if (p) p.freeze();
    try {
      const { nodes, links } = bundleRef.current;
      const s = sessionRef.current;
      const rKey = `${s.getDocumentContentVersion()}|${gridStepRef.current}`;
      if (lastReconciledKeyRef.current !== rKey) {
        lastReconciledKeyRef.current = rKey;
        reconcileGraph(gr, nodes, links, gridStepRef.current);
      }
      paintAllRef.current();
    } finally {
      if (p) p.unfreeze();
      setTimeout(() => {
        applyingSessionGraphRef.current = false;
      }, 0);
    }
  }, [revision, structureGridSize]);

  useEffect(() => {
    const gr = graphRef.current;
    const p = paperRef.current;
    if (!gr) return;
    if (p) p.freeze();
    try {
      paintAllRef.current();
    } finally {
      if (p) p.unfreeze();
    }
  }, [schemeId, errKey]);

  useEffect(() => {
    if (readOnly) return;
    const key = (e: KeyboardEvent) => {
      const root = rootRef.current;
      if (!root || !root.contains(document.activeElement)) return;
      const gph = graphRef.current;
      if (!gph) return;
      if (e.key === "Delete") {
        const ids = sessionRef.current.getStructureSelectionIds();
        if (!ids.length) return;
        const nodeIds: string[] = [];
        const linkIds: string[] = [];
        for (const id of ids) {
          const c = gph.getCell(id);
          if (!c) continue;
          if (c.isElement()) {
            nodeIds.push(id);
            gph.getConnectedLinks(c as dia.Element).forEach((l) => linkIds.push(l.id as string));
          } else if (c.isLink()) linkIds.push(id);
        }
        sessionRef.current.applyDeleteElements(nodeIds, [...new Set(linkIds)]);
        sessionRef.current.setStructurePointerSelection(null, false);
        selectedRef.current = new Set();
        return;
      }
      const step = e.shiftKey ? 10 : 1;
      if (e.key === "ArrowLeft") {
        e.preventDefault();
        sessionRef.current.applyNudgeSelected({ x: -step, y: 0 });
      } else if (e.key === "ArrowRight") {
        e.preventDefault();
        sessionRef.current.applyNudgeSelected({ x: step, y: 0 });
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        sessionRef.current.applyNudgeSelected({ x: 0, y: -step });
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        sessionRef.current.applyNudgeSelected({ x: 0, y: step });
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a") {
        e.preventDefault();
        sessionRef.current.selectAllStructureNodes();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "c") {
        e.preventDefault();
        sessionRef.current.copyStructureSelection();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "v") {
        e.preventDefault();
        sessionRef.current.pasteStructureClipboard();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "d") {
        e.preventDefault();
        sessionRef.current.duplicateStructureSelection();
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [readOnly, revision]);

  return (
    <div
      ref={rootRef}
      tabIndex={-1}
      className="relative w-full h-full overflow-hidden outline-none"
      onMouseDown={(ev) => {
        (ev.currentTarget as HTMLDivElement).focus();
        setCtxMenu((c) => ({ ...c, visible: false }));
      }}
      onContextMenu={(ev) => {
        if (readOnly) return;
        ev.preventDefault();
        const items: ContextMenuItem[] = [
          {
            id: "paste",
            label: "Paste",
            onClick: () => session.pasteStructureClipboard(),
          },
          {
            id: "select-all",
            label: "Select all",
            onClick: () => session.selectAllStructureNodes(),
          },
        ];
        setCtxMenu({ visible: true, x: ev.clientX, y: ev.clientY, items });
      }}
    >
      <ContextMenu
        visible={ctxMenu.visible}
        x={ctxMenu.x}
        y={ctxMenu.y}
        items={ctxMenu.items}
        onClose={() => setCtxMenu((c) => ({ ...c, visible: false }))}
      />
      {showMiniMap && <JointMiniMap paper={paperRef.current} graph={graphRef.current} />}
    </div>
  );
}
