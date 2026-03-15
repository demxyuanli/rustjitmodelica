/**
 * Automatic layout algorithms for structure diagram (nodes + edges).
 * All functions return positions keyed by node id. Uses default node size for spacing.
 */

const DEFAULT_NODE_WIDTH = 160;
const DEFAULT_NODE_HEIGHT = 80;
const LAYOUT_GAP_X = 80;
const LAYOUT_GAP_Y = 60;

export type DiagramLayoutKind = "grid" | "hierarchical" | "circular" | "force" | "horizontal" | "vertical";

export interface LayoutInput {
  nodeIds: string[];
  edges: { source: string; target: string }[];
}

export interface GridLayoutOptions {
  columns?: number;
  gapX?: number;
  gapY?: number;
  startX?: number;
  startY?: number;
}

export interface HierarchicalLayoutOptions {
  direction?: "TB" | "LR" | "BT" | "RL";
  layerGap?: number;
  nodeGap?: number;
  startX?: number;
  startY?: number;
}

export interface CircularLayoutOptions {
  centerX?: number;
  centerY?: number;
  radius?: number;
}

export interface ForceLayoutOptions {
  width?: number;
  height?: number;
  iterations?: number;
  repulsion?: number;
  attraction?: number;
}

export type LayoutResult = Record<string, { x: number; y: number }>;

function getInEdges(nodeId: string, edges: { source: string; target: string }[]): number {
  return edges.filter((e) => e.target === nodeId).length;
}

function topologicalLayers(
  nodeIds: string[],
  edges: { source: string; target: string }[]
): string[][] {
  const idSet = new Set(nodeIds);
  const inDegree: Record<string, number> = {};
  nodeIds.forEach((id) => (inDegree[id] = getInEdges(id, edges)));
  const layers: string[][] = [];
  const assigned = new Set<string>();

  while (assigned.size < nodeIds.length) {
    const layer: string[] = [];
    for (const id of nodeIds) {
      if (assigned.has(id)) continue;
      const deps = edges.filter((e) => e.target === id).map((e) => e.source);
      const allDepsAssigned = deps.every((d) => assigned.has(d) || !idSet.has(d));
      if (allDepsAssigned && (deps.length === 0 || deps.some((d) => assigned.has(d)))) {
        layer.push(id);
      }
    }
    if (layer.length === 0) {
      for (const id of nodeIds) {
        if (!assigned.has(id)) {
          layer.push(id);
          break;
        }
      }
    }
    layer.forEach((id) => assigned.add(id));
    if (layer.length > 0) layers.push(layer);
  }
  return layers;
}

function countCrossings(
  layers: string[][],
  edges: { source: string; target: string }[]
): number {
  const orderInLayer: Record<string, number> = {};
  layers.forEach((layer) => {
    layer.forEach((id, i) => {
      orderInLayer[id] = i;
    });
  });
  const layerOf: Record<string, number> = {};
  layers.forEach((layer, layerIdx) => {
    layer.forEach((id) => {
      layerOf[id] = layerIdx;
    });
  });
  let crossings = 0;
  for (let i = 0; i < edges.length; i++) {
    for (let j = i + 1; j < edges.length; j++) {
      const a = edges[i].source;
      const b = edges[i].target;
      const c = edges[j].source;
      const d = edges[j].target;
      const la = layerOf[a];
      const lb = layerOf[b];
      const lc = layerOf[c];
      const ld = layerOf[d];
      if (la === lc || lb === ld) continue;
      const top1 = Math.min(la, lb);
      const top2 = Math.min(lc, ld);
      const bot1 = Math.max(la, lb);
      const bot2 = Math.max(lc, ld);
      if (top1 !== top2 || bot1 !== bot2) continue;
      const upper = la < lb ? a : b;
      const lower = la < lb ? b : a;
      const upper2 = lc < ld ? c : d;
      const lower2 = lc < ld ? d : c;
      const oUpper = orderInLayer[upper];
      const oLower = orderInLayer[lower];
      const oUpper2 = orderInLayer[upper2];
      const oLower2 = orderInLayer[lower2];
      if ((oUpper < oUpper2 && oLower > oLower2) || (oUpper > oUpper2 && oLower < oLower2)) {
        crossings++;
      }
    }
  }
  return crossings;
}

/**
 * Reorder nodes within each layer to minimize edge crossings (barycentric / median heuristic).
 */
function minimizeCrossings(
  layers: string[][],
  edges: { source: string; target: string }[]
): string[][] {
  const idSet = new Set<string>();
  layers.forEach((layer) => layer.forEach((id) => idSet.add(id)));
  const layerOf: Record<string, number> = {};
  layers.forEach((layer, li) => {
    layer.forEach((id) => {
      layerOf[id] = li;
    });
  });
  const getPredIndices = (id: string): number[] => {
    const li = layerOf[id];
    return edges
      .filter((e) => e.target === id && idSet.has(e.source) && layerOf[e.source] < li)
      .map((e) => orderInLayer[e.source]);
  };
  const getSuccIndices = (id: string): number[] => {
    const li = layerOf[id];
    return edges
      .filter((e) => e.source === id && idSet.has(e.target) && layerOf[e.target] > li)
      .map((e) => orderInLayer[e.target]);
  };

  let orderInLayer: Record<string, number> = {};
  layers.forEach((layer) => {
    layer.forEach((id, i) => (orderInLayer[id] = i));
  });

  const maxPasses = 24;
  for (let pass = 0; pass < maxPasses; pass++) {
    let improved = false;
    for (let li = 0; li < layers.length; li++) {
      const layer = layers[li];
      const withBary: { id: string; bary: number }[] = layer.map((id) => {
        const pred = getPredIndices(id);
        const succ = getSuccIndices(id);
        const all = [...pred, ...succ];
        const bary = all.length === 0 ? orderInLayer[id] : all.reduce((s, v) => s + v, 0) / all.length;
        return { id, bary };
      });
      const dir = pass % 2 === 0 ? 1 : -1;
      withBary.sort((a, b) => dir * (a.bary - b.bary));
      const newOrder = withBary.map((x) => x.id);
      const before = countCrossings(layers, edges);
      layers[li] = newOrder;
      newOrder.forEach((id, i) => (orderInLayer[id] = i));
      const after = countCrossings(layers, edges);
      if (after < before) improved = true;
    }
    if (!improved) break;
  }
  return layers;
}

export function layoutGrid(
  input: LayoutInput,
  options: GridLayoutOptions = {}
): LayoutResult {
  const { nodeIds } = input;
  const cols = options.columns ?? Math.ceil(Math.sqrt(nodeIds.length));
  const gapX = options.gapX ?? LAYOUT_GAP_X;
  const gapY = options.gapY ?? LAYOUT_GAP_Y;
  const startX = options.startX ?? 0;
  const startY = options.startY ?? 0;
  const result: LayoutResult = {};
  nodeIds.forEach((id, i) => {
    const col = i % cols;
    const row = Math.floor(i / cols);
    result[id] = {
      x: startX + col * (DEFAULT_NODE_WIDTH + gapX),
      y: startY + row * (DEFAULT_NODE_HEIGHT + gapY),
    };
  });
  return result;
}

export function layoutHierarchical(
  input: LayoutInput,
  options: HierarchicalLayoutOptions = {}
): LayoutResult {
  const { nodeIds, edges } = input;
  const direction = options.direction ?? "TB";
  const layerGap = options.layerGap ?? DEFAULT_NODE_HEIGHT + LAYOUT_GAP_Y;
  const nodeGap = options.nodeGap ?? DEFAULT_NODE_WIDTH + LAYOUT_GAP_X;
  const startX = options.startX ?? 0;
  const startY = options.startY ?? 0;

  let layers = topologicalLayers(nodeIds, edges);
  layers = minimizeCrossings(layers, edges);
  const result: LayoutResult = {};

  layers.forEach((layer, layerIdx) => {
    const layerWidth = (layer.length - 1) * nodeGap;
    layer.forEach((id, i) => {
      const x = startX + (layer.length === 1 ? 0 : (i / Math.max(1, layer.length - 1)) * layerWidth);
      const y = startY + layerIdx * layerGap;
      if (direction === "TB") result[id] = { x, y };
      else if (direction === "LR") result[id] = { x: y, y: x };
      else if (direction === "BT") result[id] = { x, y: startY + (layers.length - 1 - layerIdx) * layerGap };
      else result[id] = { x: startY + (layers.length - 1 - layerIdx) * layerGap, y: x };
    });
  });

  if (direction === "LR" || direction === "RL") {
    const swap = (r: LayoutResult): LayoutResult => {
      const out: LayoutResult = {};
      Object.entries(r).forEach(([id, p]) => { out[id] = { x: p.y, y: p.x }; });
      return out;
    };
    return swap(result);
  }
  return result;
}

/**
 * Order node ids so that connected nodes are adjacent (reduces crossings on circle).
 */
function circularOrderByConnectivity(
  nodeIds: string[],
  edges: { source: string; target: string }[]
): string[] {
  const idSet = new Set(nodeIds);
  const neighbors: Record<string, string[]> = {};
  nodeIds.forEach((id) => (neighbors[id] = []));
  edges.forEach((e) => {
    if (idSet.has(e.source) && idSet.has(e.target) && e.source !== e.target) {
      if (!neighbors[e.source].includes(e.target)) neighbors[e.source].push(e.target);
      if (!neighbors[e.target].includes(e.source)) neighbors[e.target].push(e.source);
    }
  });
  const placed = new Set<string>();
  const order: string[] = [];
  const start = nodeIds[0];
  order.push(start);
  placed.add(start);
  while (order.length < nodeIds.length) {
    let bestNext: string | null = null;
    let bestScore = -1;
    for (const id of nodeIds) {
      if (placed.has(id)) continue;
      const last = order[order.length - 1];
      const connToLast = neighbors[id].includes(last) ? 2 : 0;
      const connToPlaced = neighbors[id].filter((n) => placed.has(n)).length;
      const score = connToLast * 10 + connToPlaced;
      if (score > bestScore) {
        bestScore = score;
        bestNext = id;
      }
    }
    if (bestNext == null) {
      const remaining = nodeIds.filter((id) => !placed.has(id));
      order.push(remaining[0]);
      placed.add(remaining[0]);
    } else {
      order.push(bestNext);
      placed.add(bestNext);
    }
  }
  return order;
}

export function layoutCircular(
  input: LayoutInput,
  options: CircularLayoutOptions = {}
): LayoutResult {
  const { nodeIds, edges } = input;
  const ordered = circularOrderByConnectivity(nodeIds, edges);
  const n = ordered.length;
  const centerX = options.centerX ?? (n * (DEFAULT_NODE_WIDTH + LAYOUT_GAP_X)) / 2;
  const centerY = options.centerY ?? (n * (DEFAULT_NODE_HEIGHT + LAYOUT_GAP_Y)) / 2;
  const radius = options.radius ?? Math.max(120, (n * 80) / (2 * Math.PI));
  const result: LayoutResult = {};
  ordered.forEach((id, i) => {
    const angle = n <= 1 ? 0 : (2 * Math.PI * i) / n - Math.PI / 2;
    result[id] = {
      x: centerX + radius * Math.cos(angle),
      y: centerY + radius * Math.sin(angle),
    };
  });
  return result;
}

export function layoutForce(
  input: LayoutInput,
  options: ForceLayoutOptions = {}
): LayoutResult {
  const { nodeIds, edges } = input;
  const iterations = options.iterations ?? 150;
  const repulsion = options.repulsion ?? 800;
  const attraction = options.attraction ?? 0.05;
  const width = options.width ?? 800;
  const height = options.height ?? 600;

  const pos: Record<string, { x: number; y: number }> = {};
  const idToIdx: Record<string, number> = {};
  nodeIds.forEach((id, i) => {
    idToIdx[id] = i;
    pos[id] = { x: width * (0.2 + 0.6 * Math.random()), y: height * (0.2 + 0.6 * Math.random()) };
  });

  for (let iter = 0; iter < iterations; iter++) {
    const forces: Record<string, { fx: number; fy: number }> = {};
    nodeIds.forEach((id) => (forces[id] = { fx: 0, fy: 0 }));

    for (let i = 0; i < nodeIds.length; i++) {
      for (let j = i + 1; j < nodeIds.length; j++) {
        const a = nodeIds[i];
        const b = nodeIds[j];
        const dx = pos[b].x - pos[a].x;
        const dy = pos[b].y - pos[a].y;
        const dist = Math.sqrt(dx * dx + dy * dy) || 0.1;
        const rep = (repulsion / (dist * dist)) * (DEFAULT_NODE_WIDTH / dist);
        forces[a].fx -= rep * dx;
        forces[a].fy -= rep * dy;
        forces[b].fx += rep * dx;
        forces[b].fy += rep * dy;
      }
    }

    edges.forEach((e) => {
      const dx = pos[e.target].x - pos[e.source].x;
      const dy = pos[e.target].y - pos[e.source].y;
      const dist = Math.sqrt(dx * dx + dy * dy) || 0.1;
      const force = dist * attraction;
      forces[e.source].fx += force * dx;
      forces[e.source].fy += force * dy;
      forces[e.target].fx -= force * dx;
      forces[e.target].fy -= force * dy;
    });

    nodeIds.forEach((id) => {
      pos[id].x += Math.max(-20, Math.min(20, forces[id].fx));
      pos[id].y += Math.max(-20, Math.min(20, forces[id].fy));
      pos[id].x = Math.max(0, Math.min(width, pos[id].x));
      pos[id].y = Math.max(0, Math.min(height, pos[id].y));
    });
  }

  return pos;
}

function getSourceSinkMiddle(
  nodeIds: string[],
  edges: { source: string; target: string }[]
): { sourceNodes: string[]; sinkNodes: string[]; middleNodes: string[] } {
  const hasIncoming = new Set(edges.map((e) => e.target));
  const hasOutgoing = new Set(edges.map((e) => e.source));
  const sourceNodes: string[] = [];
  const sinkNodes: string[] = [];
  const middleNodes: string[] = [];
  for (const id of nodeIds) {
    const incoming = hasIncoming.has(id);
    const outgoing = hasOutgoing.has(id);
    if (!incoming && outgoing) sourceNodes.push(id);
    else if (incoming && !outgoing) sinkNodes.push(id);
    else middleNodes.push(id);
  }
  return { sourceNodes, sinkNodes, middleNodes };
}

export interface FlowDirectionLayoutOptions {
  layerGap?: number;
  nodeGap?: number;
  startX?: number;
  startY?: number;
}

/**
 * Horizontal layout: start (source) nodes left, end (sink) nodes right, middle in flow order.
 */
export function layoutHorizontal(
  input: LayoutInput,
  options: FlowDirectionLayoutOptions = {}
): LayoutResult {
  const { nodeIds, edges } = input;
  const layerGap = options.layerGap ?? DEFAULT_NODE_WIDTH + LAYOUT_GAP_X;
  const nodeGap = options.nodeGap ?? DEFAULT_NODE_HEIGHT + LAYOUT_GAP_Y;
  const startX = options.startX ?? 0;
  const startY = options.startY ?? 0;

  const { sourceNodes, sinkNodes, middleNodes } = getSourceSinkMiddle(nodeIds, edges);
  const middleLayers = topologicalLayers(middleNodes, edges);
  const middleLayersFiltered = middleLayers.filter((layer) => layer.length > 0);
  const allLayers: string[][] = [];
  if (sourceNodes.length > 0) allLayers.push(sourceNodes);
  allLayers.push(...middleLayersFiltered);
  if (sinkNodes.length > 0) allLayers.push(sinkNodes);

  const result: LayoutResult = {};
  allLayers.forEach((layer, layerIdx) => {
    const colX = startX + layerIdx * layerGap;
    const layerHeight = (layer.length - 1) * nodeGap;
    layer.forEach((id, i) => {
      const y = layer.length === 1 ? startY : startY + (i / Math.max(1, layer.length - 1)) * layerHeight;
      result[id] = { x: colX, y };
    });
  });
  return result;
}

/**
 * Vertical layout: start (source) nodes top, end (sink) nodes bottom, middle in flow order.
 */
export function layoutVertical(
  input: LayoutInput,
  options: FlowDirectionLayoutOptions = {}
): LayoutResult {
  const { nodeIds, edges } = input;
  const layerGap = options.layerGap ?? DEFAULT_NODE_HEIGHT + LAYOUT_GAP_Y;
  const nodeGap = options.nodeGap ?? DEFAULT_NODE_WIDTH + LAYOUT_GAP_X;
  const startX = options.startX ?? 0;
  const startY = options.startY ?? 0;

  const { sourceNodes, sinkNodes, middleNodes } = getSourceSinkMiddle(nodeIds, edges);
  const middleLayers = topologicalLayers(middleNodes, edges);
  const middleLayersFiltered = middleLayers.filter((layer) => layer.length > 0);
  const allLayers: string[][] = [];
  if (sourceNodes.length > 0) allLayers.push(sourceNodes);
  allLayers.push(...middleLayersFiltered);
  if (sinkNodes.length > 0) allLayers.push(sinkNodes);

  const result: LayoutResult = {};
  allLayers.forEach((layer, layerIdx) => {
    const rowY = startY + layerIdx * layerGap;
    const layerWidth = (layer.length - 1) * nodeGap;
    layer.forEach((id, i) => {
      const x = layer.length === 1 ? startX : startX + (i / Math.max(1, layer.length - 1)) * layerWidth;
      result[id] = { x, y: rowY };
    });
  });
  return result;
}

export function applyDiagramLayout(
  kind: DiagramLayoutKind,
  input: LayoutInput,
  opts?: GridLayoutOptions & HierarchicalLayoutOptions & CircularLayoutOptions & ForceLayoutOptions & FlowDirectionLayoutOptions
): LayoutResult {
  switch (kind) {
    case "grid":
      return layoutGrid(input, opts);
    case "hierarchical":
      return layoutHierarchical(input, opts);
    case "circular":
      return layoutCircular(input, opts);
    case "force":
      return layoutForce(input, opts);
    case "horizontal":
      return layoutHorizontal(input, opts);
    case "vertical":
      return layoutVertical(input, opts);
    default:
      return layoutGrid(input, opts);
  }
}
