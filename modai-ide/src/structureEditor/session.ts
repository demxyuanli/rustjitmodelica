import type { CoordinateSystem, GraphicItem } from "../components/diagramGraphicTypes";
import {
  attachFlowRoles,
  buildUndoHistoryKey,
  diagramToNodes,
  documentToDiagram,
  nodeAndHandleToPath,
  pathToNodeAndHandle,
  roundCoord,
  uniqueInstanceName,
  type DiagramDocument,
} from "./docSync";
import type { ComponentData, ConnectionData, DiagramLink, DiagramNode, LayoutPoint } from "./types";
import { MAX_UNDO, MERGE_MS } from "./layoutConstants";
import { snapPointToGridStrict } from "../utils/gridSnap";

function cloneDoc(d: DiagramDocument): DiagramDocument {
  return structuredClone(d);
}

function remapConnPath(path: string, map: Map<string, string>): string {
  const { nodeId, handleId } = pathToNodeAndHandle(path);
  const nn = map.get(nodeId) ?? nodeId;
  return nodeAndHandleToPath(nn, handleId);
}

type EditorMode = "diagram" | "icon";

type UndoEntry = { doc: DiagramDocument; ts: number };

export type StructureSnapMode = "none" | "grid" | "gridAndGuide";

type StructureClipboardPayload = {
  components: ComponentData[];
  connections: ConnectionData[];
  layout: Record<string, LayoutPoint>;
};

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
  /** Monotonic counter for document content; used to bind skip-next-undo to a specific doc generation. */
  private docContentVersion = 0;
  private skipUndoAtVersion = -1;

  private structureSnapMode: StructureSnapMode = "grid";
  private structureGridSize = 10;
  private structureClipboard: StructureClipboardPayload | null = null;

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

  setStructureMultiSelection(ids: string[]) {
    this.selection = [...ids];
    this.emit();
  }

  setStructureSnapOptions(opts: { mode?: StructureSnapMode; gridSize?: number }) {
    if (opts.mode !== undefined) this.structureSnapMode = opts.mode;
    if (opts.gridSize !== undefined) this.structureGridSize = Math.max(1, opts.gridSize);
  }

  getStructureSelectionId() {
    return this.selection[0] ?? null;
  }

  clearUndo() {
    this.undoStack = [];
    this.redoStack = [];
    this.lastUndoKey = "";
    this.skipUndoPush = false;
    this.skipUndoAtVersion = -1;
  }

  setSkipNextUndoPush() {
    this.skipUndoPush = true;
    this.skipUndoAtVersion = this.docContentVersion;
  }

  private bumpDocContentVersion() {
    this.docContentVersion++;
  }

  private pushUndo(nodes: DiagramNode[], links: DiagramLink[]) {
    if (!this.doc) return;
    if (this.skipUndoPush) {
      if (this.skipUndoAtVersion === this.docContentVersion) {
        this.skipUndoPush = false;
        this.skipUndoAtVersion = -1;
        this.lastUndoKey = buildUndoHistoryKey(nodes, links);
        return;
      }
      this.skipUndoPush = false;
      this.skipUndoAtVersion = -1;
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
    this.docContentVersion = 0;
    this.bumpDocContentVersion();
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
    this.docContentVersion = 0;
    this.bumpDocContentVersion();
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
    this.docContentVersion = 0;
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
    let next = p;
    if (this.structureSnapMode === "grid" || this.structureSnapMode === "gridAndGuide") {
      next = snapPointToGridStrict(p, this.structureGridSize);
    }
    const prev = this.doc.graphical.layout?.[id];
    if (
      prev &&
      roundCoord(prev.x) === roundCoord(next.x) &&
      roundCoord(prev.y) === roundCoord(next.y)
    ) {
      return;
    }
    const layout = { ...(this.doc.graphical.layout ?? {}), [id]: next };
    this.doc = { ...this.doc, graphical: { ...this.doc.graphical, layout } };
    this.bumpDocContentVersion();
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.afterTopo(nodes, links);
  }

  applyConnect(src: string, sp: string, tgt: string, tp: string) {
    if (!this.doc) return;
    const from = nodeAndHandleToPath(src, sp);
    const to = nodeAndHandleToPath(tgt, tp);
    this.doc = { ...this.doc, connections: [...this.doc.connections, { from, to }] };
    this.bumpDocContentVersion();
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
    this.bumpDocContentVersion();
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
    this.bumpDocContentVersion();
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.afterTopo(nodes, links);
  }

  applyLayoutPositions(positions: Record<string, LayoutPoint>) {
    if (!this.doc) return;
    const layout = { ...(this.doc.graphical.layout ?? {}), ...positions };
    this.doc = { ...this.doc, graphical: { ...this.doc.graphical, layout } };
    this.bumpDocContentVersion();
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
    this.bumpDocContentVersion();
    this.emit();
  }

  applyComponentDeclaredType(compName: string, nextTypeName: string) {
    if (!this.doc) return;
    const trimmed = nextTypeName.trim();
    if (!trimmed) return;
    const components = this.doc.components.map((c) => (c.name !== compName ? c : { ...c, typeName: trimmed }));
    this.doc = { ...this.doc, components };
    this.bumpDocContentVersion();
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.afterTopo(nodes, links);
  }

  applyComponentAnnotationFlags(
    compName: string,
    patch: { condition?: string | null; visible?: boolean | null },
  ) {
    if (!this.doc) return;
    const components = this.doc.components.map((c) => {
      if (c.name !== compName) return c;
      let next = { ...c };
      if (patch.condition !== undefined) {
        const v = patch.condition?.trim();
        next = v ? { ...next, condition: v } : { ...next, condition: undefined };
      }
      if (patch.visible !== undefined) {
        next =
          patch.visible === null ? { ...next, visible: undefined } : { ...next, visible: Boolean(patch.visible) };
      }
      return next;
    });
    this.doc = { ...this.doc, components };
    this.bumpDocContentVersion();
    this.emit();
  }

  setCoordinateSystemForMode(mode: EditorMode, patch: Partial<CoordinateSystem>) {
    if (!this.doc) return;
    if (mode === "icon") {
      const prev = this.doc.graphical.iconAnnotation ?? { graphics: [] };
      const prevCs = prev.coordinateSystem ?? {};
      const coordinateSystem = { ...prevCs, ...patch };
      this.doc = {
        ...this.doc,
        graphical: {
          ...this.doc.graphical,
          iconAnnotation: {
            ...prev,
            coordinateSystem,
          },
        },
      };
    } else {
      const prev = this.doc.graphical.diagramAnnotation ?? { graphics: [] };
      const prevCs = prev.coordinateSystem ?? {};
      const coordinateSystem = { ...prevCs, ...patch };
      this.doc = {
        ...this.doc,
        graphical: {
          ...this.doc.graphical,
          diagramAnnotation: {
            ...prev,
            coordinateSystem,
          },
        },
      };
    }
    this.bumpDocContentVersion();
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
    this.bumpDocContentVersion();
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
    const connections = this.doc.connections.map((c) => {
      if (c.from !== fp || c.to !== tp) return c;
      if (!vertices.length) return { ...c, line: undefined };
      return { ...c, line: { ...(c.line ?? {}), points: vertices } };
    });
    this.doc = { ...this.doc, connections };
    this.bumpDocContentVersion();
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

  applyLinkRouting(linkId: string, router: string) {
    if (!this.doc) return;
    const { links } = this.getNodesLinksForCanvas(() => {});
    const link = links.find((l) => l.id === linkId);
    if (!link) return;
    const fp = nodeAndHandleToPath(link.source, link.sourcePort);
    const tp = nodeAndHandleToPath(link.target, link.targetPort);
    const connections = this.doc.connections.map((c) => {
      if (c.from !== fp || c.to !== tp) return c;
      const pts = c.line?.points ?? [];
      if (!pts.length) return c;
      return { ...c, line: { ...(c.line ?? {}), points: pts, routing: router } };
    });
    this.doc = { ...this.doc, connections };
    this.bumpDocContentVersion();
    const next = this.getNodesLinksForCanvas(() => {});
    this.afterTopo(next.nodes, next.links);
  }

  copyStructureSelection() {
    if (!this.doc) return;
    const sel = new Set(this.selection);
    const comps = this.doc.components.filter((c) => sel.has(c.name));
    if (!comps.length) {
      this.structureClipboard = null;
      return;
    }
    const nameSet = new Set(comps.map((c) => c.name));
    const conns = this.doc.connections.filter((c) => {
      const f = pathToNodeAndHandle(c.from).nodeId;
      const t = pathToNodeAndHandle(c.to).nodeId;
      return nameSet.has(f) && nameSet.has(t);
    });
    const layout: Record<string, LayoutPoint> = {};
    const allLayout = this.doc.graphical.layout ?? {};
    for (const c of comps) {
      const p = allLayout[c.name];
      if (p) layout[c.name] = { ...p };
    }
    this.structureClipboard = {
      components: structuredClone(comps),
      connections: structuredClone(conns),
      layout,
    };
  }

  pasteStructureClipboard(offset: LayoutPoint = { x: 24, y: 24 }) {
    if (!this.doc || !this.structureClipboard) return;
    const existing = this.doc.components.map((c) => c.name);
    const idMap = new Map<string, string>();
    for (const c of this.structureClipboard.components) {
      idMap.set(c.name, uniqueInstanceName(c.typeName, [...existing, ...idMap.values()]));
    }
    const newComps: ComponentData[] = this.structureClipboard.components.map((c) => ({
      ...structuredClone(c),
      name: idMap.get(c.name)!,
    }));
    const newConns: ConnectionData[] = this.structureClipboard.connections.map((c) => ({
      ...structuredClone(c),
      from: remapConnPath(c.from, idMap),
      to: remapConnPath(c.to, idMap),
    }));
    const layout = { ...(this.doc.graphical.layout ?? {}) };
    for (const c of this.structureClipboard.components) {
      const oldId = c.name;
      const nid = idMap.get(oldId)!;
      const p = this.structureClipboard.layout[oldId] ?? layout[oldId] ?? { x: 0, y: 0 };
      layout[nid] = { x: p.x + offset.x, y: p.y + offset.y };
    }
    this.doc = {
      ...this.doc,
      components: [...this.doc.components, ...newComps],
      connections: [...this.doc.connections, ...newConns],
      graphical: { ...this.doc.graphical, layout },
    };
    this.bumpDocContentVersion();
    this.selection = newComps.map((c) => c.name);
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.afterTopo(nodes, links);
  }

  duplicateStructureSelection(offset: LayoutPoint = { x: 24, y: 24 }) {
    this.copyStructureSelection();
    this.pasteStructureClipboard(offset);
  }

  selectAllStructureNodes() {
    if (!this.doc) return;
    this.selection = this.doc.components.map((c) => c.name);
    this.emit();
  }

  applyNudgeSelected(delta: LayoutPoint) {
    if (!this.doc) return;
    const sel = new Set(this.selection);
    const layout = { ...(this.doc.graphical.layout ?? {}) };
    let any = false;
    for (const id of sel) {
      if (!this.doc.components.some((c) => c.name === id)) continue;
      const cur = layout[id] ?? { x: 0, y: 0 };
      layout[id] = { x: cur.x + delta.x, y: cur.y + delta.y };
      any = true;
    }
    if (!any) return;
    this.doc = { ...this.doc, graphical: { ...this.doc.graphical, layout } };
    this.bumpDocContentVersion();
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.afterTopo(nodes, links);
  }

  applyRotateSelected90() {
    if (!this.doc) return;
    const sel = new Set(this.selection);
    const components = this.doc.components.map((c) => {
      if (!sel.has(c.name)) return c;
      const r = (c.rotation ?? 0) + 90;
      const nr = ((r % 360) + 360) % 360;
      return { ...c, rotation: nr };
    });
    this.doc = { ...this.doc, components };
    this.bumpDocContentVersion();
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.afterTopo(nodes, links);
  }

  applyFlipSelected(axis: "horizontal" | "vertical") {
    if (!this.doc) return;
    const sel = new Set(this.selection);
    const components = this.doc.components.map((c) => {
      if (!sel.has(c.name)) return c;
      const tr = c.placement?.transformation;
      const ext = tr?.extent;
      if (!ext) return c;
      const flipX = (x: number) => -x;
      const flipY = (y: number) => -y;
      const nextExtent =
        axis === "horizontal" ?
          { p1: { x: flipX(ext.p1.x), y: ext.p1.y }, p2: { x: flipX(ext.p2.x), y: ext.p2.y } }
        : { p1: { x: ext.p1.x, y: flipY(ext.p1.y) }, p2: { x: ext.p2.x, y: flipY(ext.p2.y) } };
      return {
        ...c,
        placement: {
          ...c.placement,
          transformation: {
            ...tr,
            extent: nextExtent,
          },
        },
      };
    });
    this.doc = { ...this.doc, components };
    this.bumpDocContentVersion();
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.afterTopo(nodes, links);
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
    this.bumpDocContentVersion();
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
    this.bumpDocContentVersion();
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.lastUndoKey = buildUndoHistoryKey(nodes, links);
    this.skipUndoPush = true;
    this.skipUndoAtVersion = this.docContentVersion;
    this.emit();
  }

  redo() {
    const item = this.redoStack.pop();
    if (!item) return;
    this.undoStack.push(item);
    this.doc = cloneDoc(item.doc);
    this.bumpDocContentVersion();
    const { nodes, links } = this.getNodesLinksForCanvas(() => {});
    this.lastUndoKey = buildUndoHistoryKey(nodes, links);
    this.skipUndoPush = true;
    this.skipUndoAtVersion = this.docContentVersion;
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
      replaceable: node.data?.replaceable,
      constrainedbyType: node.data?.constrainedbyType,
      condition: node.data?.condition,
      visible: node.data?.visible,
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
