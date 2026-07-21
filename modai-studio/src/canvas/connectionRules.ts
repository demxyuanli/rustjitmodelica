import type { CatalogEntry, DiagramDocument, DiagramEdge, DiagramNode, PortDef } from "../api/types";

export function canConnect(
  source: PortDef | undefined,
  target: PortDef | undefined,
  sourceDomain: string,
  targetDomain: string,
): { ok: boolean; reason?: string } {
  if (!source || !target) return { ok: false, reason: "missing port" };
  if (sourceDomain !== targetDomain && source.kind !== target.kind) {
    return { ok: false, reason: "domain mismatch" };
  }
  if (source.kind === "signal" && target.kind === "signal") {
    if (source.direction !== "out" || target.direction !== "in") {
      return { ok: false, reason: "signal must be out -> in" };
    }
    return { ok: true };
  }
  if (source.kind === "connector" && target.kind === "connector") {
    return { ok: true };
  }
  return { ok: false, reason: "incompatible port kinds" };
}

export function nextInstanceName(prefix: string, nodes: DiagramNode[]): string {
  let i = 1;
  const used = new Set(nodes.map((n) => n.instanceName));
  while (used.has(`${prefix}${i}`)) i += 1;
  return `${prefix}${i}`;
}

export function createNodeFromCatalog(
  entry: CatalogEntry,
  position: { x: number; y: number },
  nodes: DiagramNode[],
): DiagramNode {
  const instanceName = nextInstanceName(entry.defaultInstancePrefix, nodes);
  const isFmu = Boolean(entry.fmuAsset) || (entry.tags ?? []).includes("fmu");
  return {
    id: `n_${instanceName}_${Date.now()}`,
    instanceName,
    typeName: entry.typeName,
    domain: entry.domain,
    catalogId: entry.id,
    position,
    rotation: 0,
    params: entry.defaultParams.map((p) => ({ name: p.name, value: p.value })),
    ports: entry.ports.map((p) => ({ ...p })),
    nodeKind: isFmu ? "fmu" : "component",
    subsystemRef: null,
    plotHints: [...(entry.plotHints ?? [])],
    ui: {
      label: instanceName,
      width: 120,
      height: 48,
      fmuAsset: entry.fmuAsset ?? undefined,
      fmuGuid: entry.fmuGuid ?? undefined,
      fmuModelIdentifier: entry.fmuModelIdentifier ?? undefined,
    },
  };
}

export function portOf(node: DiagramNode, portId: string): PortDef | undefined {
  return node.ports.find((p) => p.id === portId);
}

export function findNode(diagram: DiagramDocument, id: string): DiagramNode | undefined {
  return diagram.nodes.find((n) => n.id === id);
}

export function removeNodes(diagram: DiagramDocument, ids: Set<string>): DiagramDocument {
  return {
    ...diagram,
    nodes: diagram.nodes.filter((n) => !ids.has(n.id)),
    edges: diagram.edges.filter(
      (e) => !ids.has(e.sourceNodeId) && !ids.has(e.targetNodeId),
    ),
  };
}

export function upsertEdge(diagram: DiagramDocument, edge: DiagramEdge): DiagramDocument {
  return { ...diagram, edges: [...diagram.edges, edge] };
}
