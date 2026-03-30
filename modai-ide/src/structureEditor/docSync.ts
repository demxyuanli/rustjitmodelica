import type { GraphicModelState, GraphicalDocumentModel } from "../types";
import type { IconDiagramAnnotation, LineAnnotation } from "../components/diagramGraphicTypes";
import type {
  ComponentData,
  ConnectionData,
  DiagramLink,
  DiagramModel,
  DiagramNode,
  DiagramNodeData,
  LayoutPoint,
  ParamValue,
} from "./types";
import { GRID_GAP, ROW_GAP } from "./layoutConstants";

export const DEFAULT_HANDLE_ID = "__default__";
export const DEBOUNCE_MS = 600;
export const COORD_KEY_DECIMALS = 4;

export type DiagramDocument = GraphicalDocumentModel<
  IconDiagramAnnotation,
  ComponentData,
  ConnectionData
>;

export function pathToNodeAndHandle(path: string): { nodeId: string; handleId: string } {
  const dot = path.indexOf(".");
  if (dot < 0) return { nodeId: path, handleId: DEFAULT_HANDLE_ID };
  return { nodeId: path.slice(0, dot), handleId: path.slice(dot + 1) };
}

export function nodeAndHandleToPath(nodeId: string, handleId: string): string {
  if (handleId === DEFAULT_HANDLE_ID || !handleId) return nodeId;
  return `${nodeId}.${handleId}`;
}

export function documentToDiagram(document: DiagramDocument): DiagramModel {
  return {
    modelName: document.modelName,
    components: document.components,
    connections: document.connections,
    layout: document.graphical.layout,
    diagramAnnotation: document.graphical.diagramAnnotation,
    iconAnnotation: document.graphical.iconAnnotation,
  };
}

export function diagramToNodes(
  diagram: DiagramModel,
  onDoubleClick?: (typeName: string, libraryId?: string) => void,
): { nodes: DiagramNode[]; links: DiagramLink[] } {
  const portUsage: Record<string, Set<string>> = {};
  for (const c of diagram.connections) {
    const { nodeId: fromId, handleId: fromH } = pathToNodeAndHandle(c.from);
    const { nodeId: toId, handleId: toH } = pathToNodeAndHandle(c.to);
    portUsage[fromId] = portUsage[fromId] ?? new Set();
    portUsage[fromId].add(fromH);
    portUsage[toId] = portUsage[toId] ?? new Set();
    portUsage[toId].add(toH);
  }

  const hasIncoming = new Set(diagram.connections.map((c) => pathToNodeAndHandle(c.to).nodeId));
  const hasOutgoing = new Set(diagram.connections.map((c) => pathToNodeAndHandle(c.from).nodeId));

  const layout = diagram.layout ?? {};
  const nodes: DiagramNode[] = diagram.components.map((comp, i) => {
    let position: { x: number; y: number };
    if (layout[comp.name]) {
      position = { x: layout[comp.name].x, y: layout[comp.name].y };
    } else if (comp.placement?.transformation?.extent) {
      const ext = comp.placement.transformation.extent;
      const origin = comp.placement.transformation.origin ?? { x: 0, y: 0 };
      position = {
        x: (ext.p1.x + ext.p2.x) / 2 + origin.x + 200,
        y: 200 - ((ext.p1.y + ext.p2.y) / 2 + origin.y),
      };
    } else {
      position = { x: (i % 4) * GRID_GAP, y: Math.floor(i / 4) * ROW_GAP };
    }

    const ports = portUsage[comp.name] ? Array.from(portUsage[comp.name]) : [DEFAULT_HANDLE_ID];
    const incoming = hasIncoming.has(comp.name);
    const outgoing = hasOutgoing.has(comp.name);

    return {
      id: comp.name,
      position,
      data: {
        typeName: comp.typeName,
        libraryId: comp.libraryId,
        portHandles: ports,
        icon: comp.icon,
        rotation: comp.rotation,
        params: comp.params,
        connectorKind: comp.connectorKind,
        isInput: comp.isInput,
        isOutput: comp.isOutput,
        isSourceNode: !incoming && outgoing,
        isSinkNode: incoming && !outgoing,
        onDoubleClick,
      },
    };
  });

  const links: DiagramLink[] = diagram.connections.map((conn, i) => {
    const a = pathToNodeAndHandle(conn.from);
    const b = pathToNodeAndHandle(conn.to);
    return {
      id: `e-${conn.from}-${conn.to}-${i}`,
      source: a.nodeId,
      sourcePort: a.handleId,
      target: b.nodeId,
      targetPort: b.handleId,
      vertices: conn.line?.points,
    };
  });

  return { nodes, links };
}

export function nodesToDiagram(
  nodes: DiagramNode[],
  links: DiagramLink[],
): {
  components: {
    name: string;
    typeName: string;
    libraryId?: string;
    params?: ParamValue[];
    isInput?: boolean;
    isOutput?: boolean;
  }[];
  connections: { from: string; to: string; line?: LineAnnotation }[];
  layout: Record<string, LayoutPoint>;
} {
  const components = nodes.map((n) => ({
    name: n.id,
    typeName: n.data?.typeName || "Block",
    libraryId: n.data?.libraryId as string | undefined,
    params: n.data?.params,
    isInput: Boolean(n.data?.isInput),
    isOutput: Boolean(n.data?.isOutput),
  }));
  const connections = links.map((l) => ({
    from: nodeAndHandleToPath(l.source, l.sourcePort),
    to: nodeAndHandleToPath(l.target, l.targetPort),
    line: l.vertices?.length ? { points: l.vertices } : undefined,
  }));
  const layout: Record<string, LayoutPoint> = {};
  nodes.forEach((n) => {
    layout[n.id] = { x: n.position.x, y: n.position.y };
  });
  return { components, connections, layout };
}

export function roundCoord(n: number): number {
  const f = 10 ** COORD_KEY_DECIMALS;
  return Math.round(n * f) / f;
}

export function buildDiagramSyncKey(
  nodes: DiagramNode[],
  links: DiagramLink[],
  graphical: GraphicModelState<IconDiagramAnnotation> | undefined,
): string {
  const { components, connections, layout } = nodesToDiagram(nodes, links);
  const layoutRounded: Record<string, LayoutPoint> = {};
  for (const [id, p] of Object.entries(layout)) {
    layoutRounded[id] = { x: roundCoord(p.x), y: roundCoord(p.y) };
  }
  const connectionsRounded = connections.map((c) => ({
    ...c,
    line:
      c.line?.points?.length ?
        { points: c.line.points.map((pt) => ({ x: roundCoord(pt.x), y: roundCoord(pt.y) })) }
      : undefined,
  }));
  return JSON.stringify({
    components,
    connections: connectionsRounded,
    layout: layoutRounded,
    diagramAnnotation: graphical?.diagramAnnotation ?? null,
    iconAnnotation: graphical?.iconAnnotation ?? null,
  });
}

export function buildUndoHistoryKey(nodes: DiagramNode[], links: DiagramLink[]): string {
  const stripVolatile = (data: DiagramNodeData) => {
    const { simValues: _sv, onDoubleClick: _odc, ...rest } = data;
    return rest;
  };
  return JSON.stringify({
    nodes: nodes.map((n) => ({
      id: n.id,
      position: { x: roundCoord(n.position.x), y: roundCoord(n.position.y) },
      data: stripVolatile(n.data),
    })),
    links: links.map((l) => ({
      id: l.id,
      source: l.source,
      sourcePort: l.sourcePort,
      target: l.target,
      targetPort: l.targetPort,
      vertices:
        l.vertices?.map((pt) => ({
          x: roundCoord(pt.x),
          y: roundCoord(pt.y),
        })) ?? undefined,
    })),
  });
}

export function uniqueInstanceName(typeName: string, existingIds: string[]): string {
  const rawBase = typeName.split(".").pop() ?? typeName;
  const sanitized = rawBase.replace(/[^A-Za-z0-9_]/g, "");
  const base =
    sanitized.length > 0 ? sanitized[0].toLowerCase() + sanitized.slice(1) : "c";
  const set = new Set(existingIds);
  if (!set.has(base)) return base;
  let i = 1;
  while (set.has(base + i)) i++;
  return base + i;
}

export function attachFlowRoles(nodes: DiagramNode[], links: DiagramLink[]): DiagramNode[] {
  const hasIncoming = new Set(links.map((l) => l.target));
  const hasOutgoing = new Set(links.map((l) => l.source));
  return nodes.map((node) => {
    const incoming = hasIncoming.has(node.id);
    const outgoing = hasOutgoing.has(node.id);
    return {
      ...node,
      data: {
        ...node.data,
        isSourceNode: !incoming && outgoing,
        isSinkNode: incoming && !outgoing,
      },
    };
  });
}
