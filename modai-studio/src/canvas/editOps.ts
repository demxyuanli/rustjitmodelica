import type { DiagramDocument, DiagramEdge, DiagramNode } from "../api/types";
import { canConnect, portOf, findNode } from "../canvas/connectionRules";
import { nodeRect, rerouteEdgesTouchingNodes } from "./edgePath";

export const GRID = 10;

export function snapToGrid(x: number, y: number, size = GRID): { x: number; y: number } {
  return {
    x: Math.round(x / size) * size,
    y: Math.round(y / size) * size,
  };
}

export function snapSize(n: number, size = GRID, min = size): number {
  return Math.max(min, Math.round(n / size) * size);
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

export type AlignMode = "left" | "top" | "hcenter" | "vcenter";

type Box = { id: string; x: number; y: number; w: number; h: number };

function boxesOf(nodes: DiagramNode[]): Box[] {
  return nodes.map((n) => {
    const r = nodeRect(n);
    return { id: n.id, x: r.x, y: r.y, w: r.w, h: r.h };
  });
}

function distribute(items: { id: string; pos: number; size: number }[], minGap: number): Map<string, number> {
  const sorted = [...items].sort((a, b) => a.pos - b.pos || a.id.localeCompare(b.id));
  const start = Math.min(...sorted.map((i) => i.pos));
  const end = Math.max(...sorted.map((i) => i.pos + i.size));
  const total = sorted.reduce((s, i) => s + i.size, 0);
  const gaps = Math.max(1, sorted.length - 1);
  const gap = Math.max(minGap, (end - start - total) / gaps);
  const out = new Map<string, number>();
  let cursor = start;
  for (const it of sorted) {
    out.set(it.id, cursor);
    cursor += it.size + gap;
  }
  return out;
}

function separate(items: { id: string; pos: number; size: number }[], minGap: number): Map<string, number> {
  const sorted = [...items].sort((a, b) => a.pos - b.pos || a.id.localeCompare(b.id));
  const out = new Map<string, number>();
  let prevEnd = Number.NEGATIVE_INFINITY;
  for (const it of sorted) {
    const pos = it.pos < prevEnd + minGap ? prevEnd + minGap : it.pos;
    out.set(it.id, pos);
    prevEnd = pos + it.size;
  }
  return out;
}

export function alignSelection(
  diagram: DiagramDocument,
  selectedIds: string[],
  mode: AlignMode,
): DiagramDocument {
  const idSet = new Set(selectedIds);
  const selected = diagram.nodes.filter((n) => idSet.has(n.id));
  if (selected.length < 2) return diagram;
  const boxes = boxesOf(selected);
  const minL = Math.min(...boxes.map((b) => b.x));
  const minT = Math.min(...boxes.map((b) => b.y));
  const maxR = Math.max(...boxes.map((b) => b.x + b.w));
  const maxB = Math.max(...boxes.map((b) => b.y + b.h));
  const cx = (minL + maxR) / 2;
  const cy = (minT + maxB) / 2;

  const next = new Map<string, { x: number; y: number }>();
  if (mode === "left") {
    const ys = separate(
      boxes.map((b) => ({ id: b.id, pos: b.y, size: b.h })),
      GRID,
    );
    for (const b of boxes) next.set(b.id, { x: minL, y: ys.get(b.id) ?? b.y });
  } else if (mode === "top") {
    const xs = separate(
      boxes.map((b) => ({ id: b.id, pos: b.x, size: b.w })),
      GRID,
    );
    for (const b of boxes) next.set(b.id, { x: xs.get(b.id) ?? b.x, y: minT });
  } else if (mode === "hcenter") {
    const xs = distribute(
      boxes.map((b) => ({ id: b.id, pos: b.x, size: b.w })),
      GRID,
    );
    for (const b of boxes) {
      next.set(b.id, { x: xs.get(b.id) ?? b.x, y: cy - b.h / 2 });
    }
  } else {
    const ys = distribute(
      boxes.map((b) => ({ id: b.id, pos: b.y, size: b.h })),
      GRID,
    );
    for (const b of boxes) {
      next.set(b.id, { x: cx - b.w / 2, y: ys.get(b.id) ?? b.y });
    }
  }

  const moved: DiagramDocument = {
    ...diagram,
    nodes: diagram.nodes.map((n) => {
      const p = next.get(n.id);
      if (!p) return n;
      return { ...n, position: snapToGrid(p.x, p.y) };
    }),
  };
  return rerouteEdgesTouchingNodes(moved, idSet);
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
  // Prefer longer instance names to avoid partial matches.
  const nodes = [...diagram.nodes].sort(
    (a, b) => b.instanceName.length - a.instanceName.length,
  );
  for (const n of nodes) {
    if (n.instanceName && message.includes(n.instanceName)) return n.id;
  }
  // Structure advisor messages may embed node id tokens.
  for (const n of nodes) {
    if (message.includes(n.id)) return n.id;
  }
  return null;
}
