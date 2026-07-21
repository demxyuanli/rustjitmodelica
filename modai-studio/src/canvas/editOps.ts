import type { DiagramDocument, DiagramEdge, DiagramNode } from "../api/types";
import { canConnect, portOf, findNode } from "../canvas/connectionRules";

const GRID = 16;

export function snapToGrid(x: number, y: number, size = GRID): { x: number; y: number } {
  return {
    x: Math.round(x / size) * size,
    y: Math.round(y / size) * size,
  };
}

export function copySelection(
  diagram: DiagramDocument,
  selectedIds: string[],
): { nodes: DiagramNode[]; edges: DiagramEdge[] } | null {
  if (!selectedIds.length) return null;
  const idSet = new Set(selectedIds);
  const nodes = diagram.nodes.filter((n) => idSet.has(n.id)).map((n) => structuredClone(n));
  const edges = diagram.edges
    .filter((e) => idSet.has(e.sourceNodeId) && idSet.has(e.targetNodeId))
    .map((e) => structuredClone(e));
  return { nodes, edges };
}

export function pasteClipboard(
  diagram: DiagramDocument,
  clip: { nodes: DiagramNode[]; edges: DiagramEdge[] },
  offset = 32,
): DiagramDocument {
  const idMap = new Map<string, string>();
  const used = new Set(diagram.nodes.map((n) => n.instanceName));
  const nodes = clip.nodes.map((n) => {
    const newId = `n_${n.instanceName}_${Date.now()}_${Math.floor(Math.random() * 1000)}`;
    idMap.set(n.id, newId);
    let name = `${n.instanceName}_copy`;
    let i = 1;
    while (used.has(name)) {
      name = `${n.instanceName}_copy${i}`;
      i += 1;
    }
    used.add(name);
    return {
      ...n,
      id: newId,
      instanceName: name,
      position: snapToGrid(n.position.x + offset, n.position.y + offset),
      ui: { ...n.ui, label: name },
    };
  });
  const edges = clip.edges.map((e) => ({
    ...e,
    id: `e_${Date.now()}_${Math.floor(Math.random() * 1000)}`,
    sourceNodeId: idMap.get(e.sourceNodeId) ?? e.sourceNodeId,
    targetNodeId: idMap.get(e.targetNodeId) ?? e.targetNodeId,
  }));
  return {
    ...diagram,
    nodes: [...diagram.nodes, ...nodes],
    edges: [...diagram.edges, ...edges],
  };
}

export function alignSelection(
  diagram: DiagramDocument,
  selectedIds: string[],
  mode: "left" | "top" | "hcenter" | "vcenter",
): DiagramDocument {
  const idSet = new Set(selectedIds);
  const selected = diagram.nodes.filter((n) => idSet.has(n.id));
  if (selected.length < 2) return diagram;
  const xs = selected.map((n) => n.position.x);
  const ys = selected.map((n) => n.position.y);
  const left = Math.min(...xs);
  const top = Math.min(...ys);
  const cx = (Math.min(...xs) + Math.max(...xs)) / 2;
  const cy = (Math.min(...ys) + Math.max(...ys)) / 2;
  return {
    ...diagram,
    nodes: diagram.nodes.map((n) => {
      if (!idSet.has(n.id)) return n;
      let x = n.position.x;
      let y = n.position.y;
      if (mode === "left") x = left;
      if (mode === "top") y = top;
      if (mode === "hcenter") x = cx;
      if (mode === "vcenter") y = cy;
      return { ...n, position: snapToGrid(x, y) };
    }),
  };
}

export type EdgeValidity = { id: string; ok: boolean; reason?: string };

export function validateEdges(diagram: DiagramDocument): EdgeValidity[] {
  return diagram.edges.map((e) => {
    const sn = findNode(diagram, e.sourceNodeId);
    const tn = findNode(diagram, e.targetNodeId);
    if (!sn || !tn) return { id: e.id, ok: false, reason: "missing node" };
    const d = canConnect(portOf(sn, e.sourcePortId), portOf(tn, e.targetPortId), sn.domain, tn.domain);
    return { id: e.id, ok: d.ok, reason: d.reason };
  });
}

export function extractInstanceHint(message: string, diagram: DiagramDocument): string | null {
  for (const n of diagram.nodes) {
    if (message.includes(n.instanceName)) return n.id;
  }
  return null;
}
