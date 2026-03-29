import type { GraphicItem } from "../components/DiagramSvgRenderer";
import {
  attachFlowRoles,
  buildUndoHistoryKey,
  diagramToNodes,
  documentToDiagram,
  nodeAndHandleToPath,
  pathToNodeAndHandle,
  roundCoord,
  type DiagramDocument,
} from "./docSync";
import type { DiagramLink, DiagramNode, LayoutPoint } from "./types";

const MAX_UNDO = 50;
const MERGE_MS = 300;

function cloneDoc(d: DiagramDocument): DiagramDocument {
  return structuredClone(d);
}

type EditorMode = "diagram" | "icon";

type UndoEntry = { doc: DiagramDocument; ts: number };

export class StructureGraphSession {
  private doc: DiagramDocument | null = null;
  private rev = 0;
  private sim: Record<string, Record<string, number>> | null = null;
  private selection: string[] = [];
  private subs = new Set<() => void>();
  private undoStack: UndoEntry[] = [];
  private redoStack: UndoEntry[] = [];
  private lastUndoKey = "";
  private skipUndoPush = false;

  subscribe = (fn: () => void) => {
    this.subs.add(fn);
    return () => this.subs.delete(fn);
  };

  private emit() {
    this.rev++;
    this.subs.forEach((f) => f());
  }

  getRevision() {
    return this.rev;
  }

  getDocument() {
    return this.doc;
  }

  diagramModel() {
    return this.doc ? documentToDiagram(this.doc) : null;
  }

  setSimOverlay(m: Record<string, Record<string, number>> | null) {
    if (m == null && this.sim == null) return;
    if (m != null && this.sim != null && JSON.stringify(m) === JSON.stringify(this.sim)) return;
    this.sim = m;
    this.emit();
  }

  setStructurePointerSelection(cellId: string | null, additive: boolean) {
    if (cellId == null) {
      if (this.selection.length === 0) return;
      this.selection = [];
      this.emit();
      return;
    }
    if (!additive) {
      if (this.selection.length === 1 && this.selection[0] === cellId) return;
      this.selection = [cellId];
      this.emit();
      return;
    }
    const s = new Set(this.selection);
    if (s.has(cellId)) s.delete(cellId);
    else s.add(cellId);
    this.selection = [...s];
    this.emit();
  }

  getStructureSelectionIds() {
    return [...this.selection];
  }

  getStructureSelectionId() {
    return this.selection[0] ?? null;
  }

  clearUndo() {
    this.undoStack = [];
    this.redoStack = [];
    this.lastUndoKey = "";
  }

  setSkipNextUndoPush() {
    this.skipUndoPush = true;
  }

  private pushUndo(nodes: DiagramNode[], links: DiagramLink[]) {
    if (!this.doc) return;
    if (this.skipUndoPush) {
      this.skipUndoPush = false;
      this.lastUndoKey = buildUndoHistoryKey(nodes, links);
      return;
    }
    if (nodes.length === 0 && links.length === 0) return;
    const k = buildUndoHistoryKey(nodes, links);
    if (k === this.lastUndoKey) return;
    this.lastUndoKey = k;
    const now = Date.now();
    const top = this.undoStack[this.undoStack.length - 1];
    const snap = cloneDoc(this.doc);
    if (top && this.undoStack.length >= 2 && now - top.ts < MERGE_MS) {
      top.doc = snap;
      top.ts = now;
      this.redoStack = [];
      return;
    }
    this.undoStack.push({ doc: snap, ts: now });
    if (this.undoStack.length > MAX_UNDO) this.undoStack.shift();
    this.redoStack = [];
  }

  private afterTopo(nodes: DiagramNode[], links: DiagramLink[]) {
    this.pushUndo(nodes, links);
    this.emit();
  }

  loadFromServer(d: DiagramDocument, resetUndo: boolean) {
    this.doc = cloneDoc(d);
    if (resetUndo) {
      this.clearUndo();
      this.undoStack = [{ doc: cloneDoc(d), ts: Date.now() }];
      const { nodes, links } = this.getNodesLinksForCanvas(() => {});
      this.lastUndoKey = buildUndoHistoryKey(nodes, links);
    }
    this.selection = [];
    this.emit();
  }

  applyConflictRefresh(d: DiagramDocument) {
    this.doc = cloneDoc(d);
    this.selection = [];
    this.clearUndo();
    this.undoStack = [{ doc: cloneDoc(d), ts: Date.now() }];
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.lastUndoKey = buildUndoHistoryKey(nodes, links);
    this.emit();
  }

  clearDocument() {
    this.doc = null;
    this.selection = [];
    this.sim = null;
    this.clearUndo();
    this.emit();
  }

  getNodesLinksForCanvas(onDbl?: (typeName: string, libraryId?: string) => void) {
    if (!this.doc) return { nodes: [] as DiagramNode[], links: [] as DiagramLink[] };
    const dm = documentToDiagram(this.doc);
    const raw = diagramToNodes(dm, onDbl);
    const flow = attachFlowRoles(raw.nodes, raw.links);
    const sel = new Set(this.selection);
    const sim = this.sim;
    return {
      nodes: flow.map((n) => ({
        ...n,
        selected: sel.has(n.id),
        data: {
          ...n.data,
          ...(sim?.[n.id] ? { simValues: sim[n.id] } : {}),
        },
      })),
      links: raw.links,
    };
  }

  applyNodePosition(id: string, p: LayoutPoint) {
    if (!this.doc) return;
    const prev = this.doc.graphical.layout?.[id];
    if (
      prev &&
      roundCoord(prev.x) === roundCoord(p.x) &&
      roundCoord(prev.y) === roundCoord(p.y)
    ) {
      return;
    }
    const layout = { ...(this.doc.graphical.layout ?? {}), [id]: p };
    this.doc = { ...this.doc, graphical: { ...this.doc.graphical, layout } };
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.afterTopo(nodes, links);
  }

  applyConnect(src: string, sp: string, tgt: string, tp: string) {
    if (!this.doc) return;
    const from = nodeAndHandleToPath(src, sp);
    const to = nodeAndHandleToPath(tgt, tp);
    this.doc = { ...this.doc, connections: [...this.doc.connections, { from, to }] };
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.afterTopo(nodes, links);
  }

  applyDeleteElements(nodeIds: string[], linkIds: string[]) {
    if (!this.doc) return;
    const nodeSet = new Set(nodeIds);
    const linkSet = new Set(linkIds);
    const components = this.doc.components.filter((c) => !nodeSet.has(c.name));
    const connections = this.doc.connections.filter((c, i) => {
      const lid = `e-${c.from}-${c.to}-${i}`;
      if (linkSet.has(lid)) return false;
      const fn = pathToNodeAndHandle(c.from).nodeId;
      const tn = pathToNodeAndHandle(c.to).nodeId;
      return !nodeSet.has(fn) && !nodeSet.has(tn);
    });
    const layout = { ...(this.doc.graphical.layout ?? {}) };
    nodeIds.forEach((id) => {
      delete layout[id];
    });
    this.doc = {
      ...this.doc,
      components,
      connections,
      graphical: { ...this.doc.graphical, layout },
    };
    this.selection = this.selection.filter((id) => !nodeSet.has(id) && !linkSet.has(id));
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.afterTopo(nodes, links);
  }

  applyDropComponent(payload: {
    id: string;
    typeName: string;
    libraryId?: string;
    position: LayoutPoint;
  }) {
    if (!this.doc) return;
    const comp = {
      name: payload.id,
      typeName: payload.typeName,
      libraryId: payload.libraryId,
      params: [] as { name: string; value: string }[],
    };
    const layout = { ...(this.doc.graphical.layout ?? {}), [payload.id]: payload.position };
    this.doc = {
      ...this.doc,
      components: [...this.doc.components, comp],
      graphical: { ...this.doc.graphical, layout },
    };
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.afterTopo(nodes, links);
  }

  applyLayoutPositions(positions: Record<string, LayoutPoint>) {
    if (!this.doc) return;
    const layout = { ...(this.doc.graphical.layout ?? {}), ...positions };
    this.doc = { ...this.doc, graphical: { ...this.doc.graphical, layout } };
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.afterTopo(nodes, links);
  }

  applyComponentParam(compName: string, paramName: string, value: string) {
    if (!this.doc) return;
    const components = this.doc.components.map((c) => {
      if (c.name !== compName) return c;
      const params = [...(c.params ?? [])];
      const ix = params.findIndex((p) => p.name === paramName);
      if (ix >= 0) params[ix] = { ...params[ix], value };
      else params.push({ name: paramName, value });
      return { ...c, params };
    });
    this.doc = { ...this.doc, components };
    this.emit();
  }

  applyComponentPlacement(
    name: string,
    patch: { x?: number; y?: number; rotation?: number },
  ) {
    if (!this.doc) return;
    const layout = { ...(this.doc.graphical.layout ?? {}) };
    const cur = layout[name] ?? { x: 0, y: 0 };
    layout[name] = { x: patch.x ?? cur.x, y: patch.y ?? cur.y };
    const components = this.doc.components.map((c) =>
      c.name !== name ? c : { ...c, rotation: patch.rotation ?? c.rotation },
    );
    this.doc = {
      ...this.doc,
      components,
      graphical: { ...this.doc.graphical, layout },
    };
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.afterTopo(nodes, links);
  }

  applyLinkVertices(linkId: string, vertices: LayoutPoint[]) {
    if (!this.doc) return;
    const { links } = this.getNodesLinksForCanvas(() => {});
    const link = links.find((l) => l.id === linkId);
    if (!link) return;
    const fp = nodeAndHandleToPath(link.source, link.sourcePort);
    const tp = nodeAndHandleToPath(link.target, link.targetPort);
    const connections = this.doc.connections.map((c) =>
      c.from === fp && c.to === tp ?
        { ...c, line: vertices.length ? { points: vertices } : undefined }
      : c,
    );
    this.doc = { ...this.doc, connections };
    const next = this.getNodesLinksForCanvas(() => {});
    this.afterTopo(next.nodes, next.links);
  }

  applyLinkVerticesIfChanged(linkId: string, vertices: LayoutPoint[]) {
    const { links } = this.getNodesLinksForCanvas(() => {});
    const link = links.find((l) => l.id === linkId);
    if (!link) return;
    const cur = link.vertices ?? [];
    if (cur.length !== vertices.length) {
      this.applyLinkVertices(linkId, vertices);
      return;
    }
    const f = 10 ** 4;
    const same = cur.every(
      (p, i) =>
        Math.round(p.x * f) === Math.round(vertices[i].x * f) &&
        Math.round(p.y * f) === Math.round(vertices[i].y * f),
    );
    if (!same) this.applyLinkVertices(linkId, vertices);
  }

  setGraphicsForMode(mode: EditorMode, graphics: GraphicItem[]) {
    if (!this.doc) return;
    if (mode === "icon") {
      this.doc = {
        ...this.doc,
        graphical: {
          ...this.doc.graphical,
          iconAnnotation: {
            coordinateSystem: this.doc.graphical.iconAnnotation?.coordinateSystem,
            graphics,
          },
        },
      };
    } else {
      this.doc = {
        ...this.doc,
        graphical: {
          ...this.doc.graphical,
          diagramAnnotation: {
            coordinateSystem: this.doc.graphical.diagramAnnotation?.coordinateSystem,
            graphics,
          },
        },
      };
    }
    const now = Date.now();
    const snap = cloneDoc(this.doc);
    const top = this.undoStack[this.undoStack.length - 1];
    if (top && this.undoStack.length >= 2 && now - top.ts < MERGE_MS) {
      top.doc = snap;
      top.ts = now;
      this.redoStack = [];
    } else {
      this.undoStack.push({ doc: snap, ts: now });
      if (this.undoStack.length > MAX_UNDO) this.undoStack.shift();
      this.redoStack = [];
    }
    this.emit();
  }

  canUndo() {
    return this.undoStack.length > 1;
  }

  canRedo() {
    return this.redoStack.length > 0;
  }

  undo() {
    const popped = this.undoStack.pop();
    if (!popped) return;
    this.redoStack.push(popped);
    const prev = this.undoStack[this.undoStack.length - 1];
    if (!prev) return;
    this.doc = cloneDoc(prev.doc);
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.lastUndoKey = buildUndoHistoryKey(nodes, links);
    this.skipUndoPush = true;
    this.emit();
  }

  redo() {
    const item = this.redoStack.pop();
    if (!item) return;
    this.undoStack.push(item);
    this.doc = cloneDoc(item.doc);
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.lastUndoKey = buildUndoHistoryKey(nodes, links);
    this.skipUndoPush = true;
    this.emit();
  }

  getSelectedComponentForPanel(onDbl?: (typeName: string, libraryId?: string) => void) {
    const id = this.getStructureSelectionId();
    if (!id || !this.doc) return null;
    const { nodes } = this.getNodesLinksForCanvas(onDbl);
    const node = nodes.find((n) => n.id === id);
    if (!node) return null;
    return {
      name: node.id,
      typeName: node.data?.typeName ?? "",
      libraryId: node.data?.libraryId as string | undefined,
      params: node.data?.params ?? [],
      placement: {
        transformation: {
          origin: { x: node.position.x, y: node.position.y },
          rotation: node.data?.rotation,
        },
      },
    };
  }

  activeGraphics(mode: EditorMode) {
    const d = this.diagramModel();
    if (!d) return [];
    return mode === "icon" ? (d.iconAnnotation?.graphics ?? []) : (d.diagramAnnotation?.graphics ?? []);
  }
}

export function createStructureGraphSession() {
  return new StructureGraphSession();
}
