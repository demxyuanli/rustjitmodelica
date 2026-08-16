import type { CatalogEntry, DiagramDocument, DiagramEdge, DiagramNode, PortDef } from "../api/types";
import { drawDefaultSize } from "./drawParams";

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
  const isFmu = (entry.tags ?? []).includes("fmu") || Boolean(entry.fmuGuid);
  const isDraw = entry.domain === "draw" || (entry.tags ?? []).includes("draw");
  const isTerminal = (entry.tags ?? []).includes("terminal");
  const isBusBar = (entry.tags ?? []).includes("busbar");
  const isJump = (entry.tags ?? []).includes("jump");
  const isSgTransition = (entry.tags ?? []).includes("sg-transition");
  const isSgStep = (entry.tags ?? []).includes("sg-step");
  const isModelRef =
    (entry.tags ?? []).includes("modelRef") ||
    entry.category === "ModelRef" ||
    entry.category === "UserLib";
  const moAsset = (entry.tags ?? [])
    .find((t) => t.startsWith("asset:"))
    ?.slice("asset:".length);
  const hasMoIcon = Boolean(entry.icon);
  const fanIn = entry.ports.filter((p) => p.direction === "in").length;
  const fanOut = entry.ports.filter((p) => p.direction === "out").length;
  const size = isDraw
    ? drawDefaultSize(entry.id)
    : isTerminal
      ? { width: 72, height: 36 }
        : isJump
        ? { width: 64, height: 40 }
        : isSgTransition
          ? { width: 36, height: 72 }
          : isSgStep
            ? { width: 88, height: 48 }
        : isBusBar
          ? {
              width: (entry.tags ?? []).includes("bus") ? 28 : 22,
              height: Math.max(48, 16 + Math.max(fanIn, fanOut) * 18),
            }
          : isModelRef
            ? { width: 132, height: 56 }
            : hasMoIcon
              ? { width: 148, height: 80 }
              : { width: 120, height: 48 };
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
    nodeKind: isDraw ? "draw" : isFmu ? "fmu" : "component",
    subsystemRef: null,
    plotHints: [...(entry.plotHints ?? [])],
    ui: {
      label: isDraw
        ? entry.defaultParams.find((p) => p.name === "text")?.value ?? instanceName
        : instanceName,
      width: size.width,
      height: size.height,
      fmuAsset: entry.fmuAsset ?? undefined,
      fmuGuid: entry.fmuGuid ?? undefined,
      fmuModelIdentifier: entry.fmuModelIdentifier ?? undefined,
      icon: entry.icon ?? undefined,
      moAsset: moAsset || undefined,
    },
  };
}

export function portOf(node: DiagramNode, portId: string): PortDef | undefined {
  return node.ports.find((p) => p.id === portId);
}

export type ConnectingPort = {
  nodeId: string;
  handleId: string | null;
  handleType: "source" | "target";
};

/** Ports that can legally finish a connection started from `connecting`. */
export function compatibleMagnetPorts(
  diagram: DiagramDocument,
  connecting: ConnectingPort,
): Map<string, string[]> {
  const sn = findNode(diagram, connecting.nodeId);
  if (!sn || !connecting.handleId) return new Map();
  const srcPort = portOf(sn, connecting.handleId);
  const out = new Map<string, string[]>();
  for (const n of diagram.nodes) {
    if (n.id === sn.id) continue;
    const ids: string[] = [];
    for (const p of n.ports) {
      const ok =
        connecting.handleType === "source"
          ? canConnect(srcPort, p, sn.domain, n.domain).ok
          : canConnect(p, srcPort, n.domain, sn.domain).ok;
      if (ok) ids.push(p.id);
    }
    if (ids.length) out.set(n.id, ids);
  }
  return out;
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
