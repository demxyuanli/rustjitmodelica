import { useEffect, useMemo, useRef, useSyncExternalStore } from "react";
import { dia, shapes } from "@joint/core";
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
import { DEFAULT_HANDLE_ID } from "./docSync";
import type { DiagramLink, DiagramNode, DiagramNodeData } from "./types";
import type { StructureGraphSession } from "./session";

const CELL_SIG_KEY = "seCellSig";

export interface JointStructureEditorProps {
  session: StructureGraphSession;
  readOnly: boolean;
  onDoubleClick?: (typeName: string, libraryId?: string) => void;
  onPaperReady?: (handle: JointPaperHandle | null) => void;
  showMiniMap?: boolean;
  snapToGrid?: boolean;
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

function cellSignature(node: DiagramNode): string {
  return JSON.stringify({
    id: node.id,
    x: rq(node.position.x),
    y: rq(node.position.y),
    typeName: node.data?.typeName,
    libraryId: node.data?.libraryId,
    portHandles: node.data?.portHandles,
    connectorKind: node.data?.connectorKind,
    isInput: node.data?.isInput,
    isOutput: node.data?.isOutput,
    isSourceNode: node.data?.isSourceNode,
    isSinkNode: node.data?.isSinkNode,
    rotation: node.data?.rotation,
    params: node.data?.params,
    hasError: node.data?.hasError,
    errorMessage: node.data?.errorMessage,
  });
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
  const sig = `${sel ? 1 : 0}|${hov ? 1 : 0}|${err ? 1 : 0}`;
  if (sigCache.get(elId) === sig) return;
  sigCache.set(elId, sig);
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

function reconcileGraph(g: dia.Graph, nodes: DiagramNode[], links: DiagramLink[]) {
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
          },
          g,
        ),
      );
    } else if (link.vertices?.length) {
      const jl = cell as dia.Link;
      if (vertKey(jl.vertices()) !== vertKey(link.vertices)) jl.vertices(link.vertices);
    }
  }
}

export function JointStructureEditor({
  session,
  readOnly,
  onDoubleClick,
  onPaperReady,
  showMiniMap = true,
  snapToGrid = true,
  canConnect,
  onDiagramSelection,
}: JointStructureEditorProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const graphRef = useRef<dia.Graph | null>(null);
  const paperRef = useRef<dia.Paper | null>(null);
  const sessionRef = useRef(session);
  sessionRef.current = session;
  const onDblRef = useRef(onDoubleClick);
  onDblRef.current = onDoubleClick;
  const canConnRef = useRef(canConnect);
  canConnRef.current = canConnect;
  const onSelRef = useRef(onDiagramSelection);
  onSelRef.current = onDiagramSelection;

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
    const root = rootRef.current;
    if (!root) return;
    let alive = true;
    let readyRaf: number | null = null;

    const el = document.createElement("div");
    el.style.width = "100%";
    el.style.height = "100%";
    root.appendChild(el);

    const graph = new dia.Graph({}, { cellNamespace: shapes });
    const paper = createPaper({
      el,
      graph,
      gridSize: snapToGrid ? 10 : 1,
      readOnly,
    });
    graphRef.current = graph;
    paperRef.current = paper;

    const init = sessionRef.current.getNodesLinksForCanvas(onDblRef.current);
    applyingSessionGraphRef.current = true;
    paper.freeze();
    try {
      reconcileGraph(graph, init.nodes, init.links);
      paintAllRef.current();
    } finally {
      paper.unfreeze();
      setTimeout(() => {
        applyingSessionGraphRef.current = false;
      }, 0);
    }

    paper.on("element:pointerclick", (ev: dia.ElementView, evt: dia.Event) => {
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
    });

    paper.on("blank:pointerclick", () => {
      sessionRef.current.setStructurePointerSelection(null, false);
      selectedRef.current = new Set();
      onSelRef.current?.();
    });

    paper.on("element:mouseenter", (ev: dia.ElementView) => {
      hoverRef.current = ev.model.id as string;
      scheduleHoverRef.current();
    });
    paper.on("element:mouseleave", () => {
      hoverRef.current = null;
      scheduleHoverRef.current();
    });

    paper.on("element:pointerup", (ev: dia.ElementView) => {
      if (applyingSessionGraphRef.current) return;
      const m = ev.model as dia.Element;
      const p = m.position();
      sessionRef.current.applyNodePosition(m.id as string, { x: p.x, y: p.y });
    });

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

    paper.on("element:pointerdblclick", (ev: dia.ElementView) => {
      const id = ev.model.id as string;
      const node = sessionRef.current.getNodesLinksForCanvas(onDblRef.current).nodes.find((n) => n.id === id);
      node?.data.onDoubleClick?.(node.data.typeName, node.data.libraryId as string | undefined);
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
      paper.remove();
      graphRef.current = null;
      paperRef.current = null;
    };
  }, []);

  useEffect(() => {
    const p = paperRef.current;
    if (!p) return;
    const o = p.options as { interactive?: unknown; gridSize?: number };
    o.interactive = readOnly ? false : { elementMove: true, addLinkFromMagnet: true, labelMove: false };
    o.gridSize = snapToGrid ? 10 : 1;
  }, [readOnly, snapToGrid]);

  useEffect(() => {
    const g = graphRef.current;
    const p = paperRef.current;
    if (!g) return;
    applyingSessionGraphRef.current = true;
    if (p) p.freeze();
    try {
      const { nodes, links } = bundleRef.current;
      reconcileGraph(g, nodes, links);
      paintAllRef.current();
    } finally {
      if (p) p.unfreeze();
      setTimeout(() => {
        applyingSessionGraphRef.current = false;
      }, 0);
    }
  }, [revision]);

  useEffect(() => {
    const g = graphRef.current;
    const p = paperRef.current;
    if (!g) return;
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
      if (e.key !== "Delete") return;
      const g = graphRef.current;
      if (!g) return;
      const ids = sessionRef.current.getStructureSelectionIds();
      if (!ids.length) return;
      const nodeIds: string[] = [];
      const linkIds: string[] = [];
      for (const id of ids) {
        const c = g.getCell(id);
        if (!c) continue;
        if (c.isElement()) {
          nodeIds.push(id);
          g.getConnectedLinks(c as dia.Element).forEach((l) => linkIds.push(l.id as string));
        } else if (c.isLink()) linkIds.push(id);
      }
      sessionRef.current.applyDeleteElements(nodeIds, [...new Set(linkIds)]);
      sessionRef.current.setStructurePointerSelection(null, false);
      selectedRef.current = new Set();
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [readOnly, revision]);

  return (
    <div className="relative w-full h-full overflow-hidden">
      <div ref={rootRef} className="absolute inset-0" />
      {showMiniMap && <JointMiniMap paper={paperRef.current} graph={graphRef.current} />}
    </div>
  );
}
