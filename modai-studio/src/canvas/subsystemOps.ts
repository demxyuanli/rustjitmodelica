import type {
  DiagramDocument,
  DiagramEdge,
  DiagramNode,
  PortDef,
} from "../api/types";
import { isInportNode, isOutportNode } from "./terminalNodes";

export interface CreateSubsystemResult {
  parent: DiagramDocument;
  child: DiagramDocument;
  subsystemNodeId: string;
}

type PortBinding = { nodeId: string; portId: string };

function avgPos(nodes: DiagramNode[]): { x: number; y: number } {
  if (!nodes.length) return { x: 120, y: 120 };
  const x = nodes.reduce((s, n) => s + n.position.x, 0) / nodes.length;
  const y = nodes.reduce((s, n) => s + n.position.y, 0) / nodes.length;
  return { x, y };
}

function bboxOf(nodes: DiagramNode[]): {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
} {
  if (!nodes.length) {
    return { minX: 80, minY: 80, maxX: 280, maxY: 200 };
  }
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const n of nodes) {
    const w = n.ui?.width ?? 120;
    const h = n.ui?.height ?? 48;
    minX = Math.min(minX, n.position.x);
    minY = Math.min(minY, n.position.y);
    maxX = Math.max(maxX, n.position.x + w);
    maxY = Math.max(maxY, n.position.y + h);
  }
  return { minX, minY, maxX, maxY };
}

function terminalSelfPort(direction: "in" | "out"): PortDef {
  // Inside the child: Inport sources signal (out); Outport sinks (in).
  return {
    id: "self",
    kind: "signal",
    direction: direction === "in" ? "out" : "in",
    typeName: direction === "in" ? "RealInput" : "RealOutput",
  };
}

function makeBoundaryTerminal(
  kind: "in" | "out",
  portNumber: number,
  position: { x: number; y: number },
  stamp: number,
): DiagramNode {
  const isIn = kind === "in";
  const instanceName = `${isIn ? "in" : "out"}${portNumber}`;
  return {
    id: `n_${instanceName}_${stamp}_${portNumber}`,
    instanceName,
    typeName: isIn
      ? "Modelica.Blocks.Interfaces.RealInput"
      : "Modelica.Blocks.Interfaces.RealOutput",
    domain: "signal",
    catalogId: isIn ? "blocks.terminals.inport" : "blocks.terminals.outport",
    position,
    rotation: 0,
    params: [{ name: "portNumber", value: String(portNumber) }],
    ports: [terminalSelfPort(kind)],
    nodeKind: "component",
    subsystemRef: null,
    plotHints: [],
    ui: {
      label: instanceName,
      width: 72,
      height: 36,
    },
  };
}

function sortKeyTerminal(n: DiagramNode): number {
  const v = Number(n.params.find((p) => p.name === "portNumber")?.value);
  return Number.isFinite(v) ? v : 999;
}

export function subsystemFaceSize(inCount: number, outCount: number): {
  width: number;
  height: number;
} {
  const rows = Math.max(inCount, outCount, 1);
  return {
    width: 160,
    height: Math.max(72, 28 + rows * 22),
  };
}

/** Build parent-facing ports + bindings from Inport/Outport terminals in a child diagram. */
export function buildPortsFromChildTerminals(child: DiagramDocument): {
  ports: PortDef[];
  portBindings: Record<string, PortBinding>;
  width: number;
  height: number;
} {
  const ins = child.nodes.filter(isInportNode).sort((a, b) => sortKeyTerminal(a) - sortKeyTerminal(b));
  const outs = child.nodes
    .filter(isOutportNode)
    .sort((a, b) => sortKeyTerminal(a) - sortKeyTerminal(b));

  const ports: PortDef[] = [];
  const portBindings: Record<string, PortBinding> = {};
  const used = new Set<string>();

  const uniqueId = (base: string): string => {
    let id = base;
    let n = 2;
    while (used.has(id)) {
      id = `${base}_${n++}`;
    }
    used.add(id);
    return id;
  };

  for (const n of ins) {
    const num = n.params.find((p) => p.name === "portNumber")?.value?.trim() || "1";
    const portId = uniqueId(`In${num}`);
    ports.push({
      id: portId,
      kind: "signal",
      direction: "in",
      typeName: "RealInput",
    });
    portBindings[portId] = { nodeId: n.id, portId: "self" };
  }
  for (const n of outs) {
    const num = n.params.find((p) => p.name === "portNumber")?.value?.trim() || "1";
    const portId = uniqueId(`Out${num}`);
    ports.push({
      id: portId,
      kind: "signal",
      direction: "out",
      typeName: "RealOutput",
    });
    portBindings[portId] = { nodeId: n.id, portId: "self" };
  }

  const size = subsystemFaceSize(ins.length, outs.length);
  return { ports, portBindings, ...size };
}

/** Update a subsystem node on `parent` to match terminals in `child`. */
export function syncSubsystemNodeFromChild(
  parent: DiagramDocument,
  child: DiagramDocument,
  subsystemRef: string,
): DiagramDocument {
  const sub = parent.nodes.find((n) => n.subsystemRef === subsystemRef);
  if (!sub) return parent;
  const built = buildPortsFromChildTerminals(child);
  return {
    ...parent,
    nodes: parent.nodes.map((n) =>
      n.id !== sub.id
        ? n
        : {
            ...n,
            ports: built.ports,
            ui: {
              ...n.ui,
              portBindings: built.portBindings,
              width: built.width,
              height: built.height,
            },
          },
    ),
  };
}

/** Sync every subsystem node in `root` (and nested parents in `subs`) from their children. */
export function syncAllSubsystemPorts(
  root: DiagramDocument,
  subs: Record<string, DiagramDocument>,
): { root: DiagramDocument; subsystems: Record<string, DiagramDocument> } {
  let nextRoot = root;
  const nextSubs = { ...subs };

  const syncDoc = (doc: DiagramDocument): DiagramDocument => {
    let d = doc;
    for (const n of doc.nodes) {
      if (n.nodeKind !== "subsystem" || !n.subsystemRef) continue;
      const child = n.subsystemRef === "root" ? nextRoot : nextSubs[n.subsystemRef];
      if (!child) continue;
      d = syncSubsystemNodeFromChild(d, child, n.subsystemRef);
    }
    return d;
  };

  // Children first so nested faces are current, then parents.
  for (const id of Object.keys(nextSubs)) {
    nextSubs[id] = syncDoc(nextSubs[id]!);
  }
  nextRoot = syncDoc(nextRoot);
  return { root: nextRoot, subsystems: nextSubs };
}

/**
 * Encapsulate selected nodes into a child diagram and replace them with one subsystem node.
 * Cut edges promote to Inport/Outport terminals inside the child (Simulink-like).
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

  const stamp = Date.now();
  const subId = `sub_${stamp}`;
  const instanceName = `subsystem${parent.nodes.filter((n) => n.nodeKind === "subsystem").length + 1}`;
  const box = bboxOf(selected);

  const childNodes: DiagramNode[] = selected.map((n) => structuredClone(n));
  const childEdges: DiagramEdge[] = internalEdges.map((e) => structuredClone(e));
  const ports: PortDef[] = [];
  const portBindings: Record<string, PortBinding> = {};
  const rewrittenEdges: DiagramEdge[] = [];

  let inIdx = 1;
  let outIdx = 1;

  for (const e of cutIn) {
    const portNum = inIdx++;
    const portId = `In${portNum}`;
    const target = selected.find((n) => n.id === e.targetNodeId);
    let terminal: DiagramNode;
    if (target && isInportNode(target)) {
      terminal = childNodes.find((n) => n.id === target.id)!;
      const pn = terminal.params.find((p) => p.name === "portNumber");
      if (pn) pn.value = String(portNum);
      else terminal.params.push({ name: "portNumber", value: String(portNum) });
    } else {
      const y =
        box.minY +
        ((portNum - 0.5) / Math.max(cutIn.length, 1)) * Math.max(box.maxY - box.minY, 60);
      terminal = makeBoundaryTerminal("in", portNum, { x: box.minX - 110, y }, stamp);
      childNodes.push(terminal);
      childEdges.push({
        id: `e_${terminal.id}_to_${e.targetNodeId}_${stamp}`,
        sourceNodeId: terminal.id,
        sourcePortId: "self",
        targetNodeId: e.targetNodeId,
        targetPortId: e.targetPortId,
        signalName: e.signalName ?? null,
        vertices: [],
        routing: "orthogonal",
      });
    }
    ports.push({
      id: portId,
      kind: "signal",
      direction: "in",
      typeName: "RealInput",
    });
    portBindings[portId] = { nodeId: terminal.id, portId: "self" };
    rewrittenEdges.push({
      ...e,
      id: `${e.id}_to_${subId}`,
      targetNodeId: `n_${subId}`,
      targetPortId: portId,
    });
  }

  for (const e of cutOut) {
    const portNum = outIdx++;
    const portId = `Out${portNum}`;
    const source = selected.find((n) => n.id === e.sourceNodeId);
    let terminal: DiagramNode;
    if (source && isOutportNode(source)) {
      terminal = childNodes.find((n) => n.id === source.id)!;
      const pn = terminal.params.find((p) => p.name === "portNumber");
      if (pn) pn.value = String(portNum);
      else terminal.params.push({ name: "portNumber", value: String(portNum) });
    } else {
      const y =
        box.minY +
        ((portNum - 0.5) / Math.max(cutOut.length, 1)) * Math.max(box.maxY - box.minY, 60);
      terminal = makeBoundaryTerminal("out", portNum, { x: box.maxX + 40, y }, stamp);
      childNodes.push(terminal);
      childEdges.push({
        id: `e_${e.sourceNodeId}_to_${terminal.id}_${stamp}`,
        sourceNodeId: e.sourceNodeId,
        sourcePortId: e.sourcePortId,
        targetNodeId: terminal.id,
        targetPortId: "self",
        signalName: e.signalName ?? null,
        vertices: [],
        routing: "orthogonal",
      });
    }
    ports.push({
      id: portId,
      kind: "signal",
      direction: "out",
      typeName: "RealOutput",
    });
    portBindings[portId] = { nodeId: terminal.id, portId: "self" };
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
    nodes: childNodes,
    edges: childEdges,
    annotations: [],
    viewport: { x: 0, y: 0, zoom: 1 },
  };

  const size = subsystemFaceSize(
    ports.filter((p) => p.direction === "in").length,
    ports.filter((p) => p.direction === "out").length,
  );
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
      width: size.width,
      height: size.height,
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
