import type {
  DiagramDocument,
  DiagramEdge,
  DiagramNode,
  PortDef,
} from "../api/types";

export interface CreateSubsystemResult {
  parent: DiagramDocument;
  child: DiagramDocument;
  subsystemNodeId: string;
}

function avgPos(nodes: DiagramNode[]): { x: number; y: number } {
  if (!nodes.length) return { x: 120, y: 120 };
  const x = nodes.reduce((s, n) => s + n.position.x, 0) / nodes.length;
  const y = nodes.reduce((s, n) => s + n.position.y, 0) / nodes.length;
  return { x, y };
}

/**
 * Encapsulate selected nodes into a child diagram and replace them with one subsystem node.
 */
export function createSubsystemFromSelection(
  parent: DiagramDocument,
  selectedIds: string[],
): CreateSubsystemResult | null {
  if (selectedIds.length < 1) return null;
  const idSet = new Set(selectedIds);
  const selected = parent.nodes.filter((n) => idSet.has(n.id));
  if (!selected.length) return null;

  const internalEdges = parent.edges.filter(
    (e) => idSet.has(e.sourceNodeId) && idSet.has(e.targetNodeId),
  );
  const cutIn: DiagramEdge[] = parent.edges.filter(
    (e) => !idSet.has(e.sourceNodeId) && idSet.has(e.targetNodeId),
  );
  const cutOut: DiagramEdge[] = parent.edges.filter(
    (e) => idSet.has(e.sourceNodeId) && !idSet.has(e.targetNodeId),
  );

  const subId = `sub_${Date.now()}`;
  const instanceName = `subsystem${parent.nodes.filter((n) => n.nodeKind === "subsystem").length + 1}`;
  const ports: PortDef[] = [];
  const portBindings: Record<string, { nodeId: string; portId: string }> = {};

  let inIdx = 1;
  let outIdx = 1;
  const rewrittenEdges: DiagramEdge[] = [];

  for (const e of cutIn) {
    const portId = `u${inIdx++}`;
    ports.push({
      id: portId,
      kind: "signal",
      direction: "in",
      typeName: "RealInput",
    });
    portBindings[portId] = { nodeId: e.targetNodeId, portId: e.targetPortId };
    rewrittenEdges.push({
      ...e,
      id: `${e.id}_to_${subId}`,
      targetNodeId: `n_${subId}`,
      targetPortId: portId,
    });
  }
  for (const e of cutOut) {
    const portId = `y${outIdx++}`;
    ports.push({
      id: portId,
      kind: "signal",
      direction: "out",
      typeName: "RealOutput",
    });
    portBindings[portId] = { nodeId: e.sourceNodeId, portId: e.sourcePortId };
    rewrittenEdges.push({
      ...e,
      id: `${e.id}_from_${subId}`,
      sourceNodeId: `n_${subId}`,
      sourcePortId: portId,
    });
  }

  const child: DiagramDocument = {
    schemaVersion: 1,
    modelName: instanceName,
    diagramId: subId,
    parentDiagramId: parent.diagramId,
    nodes: selected.map((n) => structuredClone(n)),
    edges: internalEdges.map((e) => structuredClone(e)),
    annotations: [],
    viewport: { x: 0, y: 0, zoom: 1 },
  };

  const pos = avgPos(selected);
  const subsystemNode: DiagramNode = {
    id: `n_${subId}`,
    instanceName,
    typeName: "Subsystem",
    domain: "signal",
    catalogId: "studio.subsystem",
    position: pos,
    rotation: 0,
    params: [],
    ports,
    nodeKind: "subsystem",
    subsystemRef: subId,
    plotHints: [],
    ui: {
      label: instanceName,
      width: 140,
      height: 56,
      portBindings,
    },
  };

  const keptNodes = parent.nodes.filter((n) => !idSet.has(n.id));
  const keptEdges = parent.edges.filter(
    (e) => !idSet.has(e.sourceNodeId) && !idSet.has(e.targetNodeId),
  );

  const nextParent: DiagramDocument = {
    ...parent,
    nodes: [...keptNodes, subsystemNode],
    edges: [...keptEdges, ...rewrittenEdges],
  };

  return { parent: nextParent, child, subsystemNodeId: subsystemNode.id };
}
