import type { DiagramDocument, DiagramNode, Position } from "../api/types";
import { isDrawNode } from "./drawParams";
import { nodeBoxSize, portAnchorOnBox } from "./portGeometry";

export type Rect = { x: number; y: number; w: number; h: number };

const PAD = 12;
const GRID = 10;
const LANE = 28;

function snap(n: number): number {
  return Math.round(n / GRID) * GRID;
}

/** Expand point list into an orthogonal polyline (HV corners). */
export function orthogonalize(points: Position[]): Position[] {
  if (points.length < 2) return points.slice();
  const out: Position[] = [{ ...points[0]! }];
  for (let i = 1; i < points.length; i++) {
    const prev = out[out.length - 1]!;
    const cur = points[i]!;
    if (Math.abs(prev.x - cur.x) < 0.5 || Math.abs(prev.y - cur.y) < 0.5) {
      out.push({ ...cur });
    } else {
      out.push({ x: cur.x, y: prev.y });
      out.push({ ...cur });
    }
  }
  return out;
}

export function pathFromPoints(points: Position[]): string {
  if (!points.length) return "";
  let d = `M ${points[0]!.x} ${points[0]!.y}`;
  for (let i = 1; i < points.length; i++) {
    d += ` L ${points[i]!.x} ${points[i]!.y}`;
  }
  return d;
}

export function edgeCenter(points: Position[]): Position {
  if (!points.length) return { x: 0, y: 0 };
  if (points.length === 1) return { ...points[0]! };
  let total = 0;
  const segLens: number[] = [];
  for (let i = 1; i < points.length; i++) {
    const a = points[i - 1]!;
    const b = points[i]!;
    const len = Math.hypot(b.x - a.x, b.y - a.y);
    segLens.push(len);
    total += len;
  }
  if (total < 1e-6) return { ...points[0]! };
  let remain = total / 2;
  for (let i = 1; i < points.length; i++) {
    const a = points[i - 1]!;
    const b = points[i]!;
    const len = segLens[i - 1]!;
    if (remain <= len) {
      const t = len < 1e-6 ? 0 : remain / len;
      return { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t };
    }
    remain -= len;
  }
  return { ...points[points.length - 1]! };
}

/** Midpoint of source->target for inserting a new vertex. */
export function defaultVertex(source: Position, target: Position): Position {
  return {
    x: snap((source.x + target.x) / 2),
    y: snap((source.y + target.y) / 2),
  };
}

export function nodeRect(n: DiagramNode): Rect {
  const { w, h } = nodeBoxSize(n);
  return { x: n.position.x, y: n.position.y, w, h };
}

function inflate(r: Rect, pad: number): Rect {
  return {
    x: r.x - pad,
    y: r.y - pad,
    w: r.w + pad * 2,
    h: r.h + pad * 2,
  };
}

/** Port anchor in diagram coords; follows 90-degree block rotation. */
export function portAnchor(
  n: DiagramNode,
  portId: string,
  role: "source" | "target",
): Position {
  return portAnchorOnBox(n, portId, role, nodeRect(n));
}

function segHitsRect(a: Position, b: Position, r: Rect): boolean {
  // Axis-aligned segment vs AABB (inclusive inflate already applied).
  const minX = Math.min(a.x, b.x);
  const maxX = Math.max(a.x, b.x);
  const minY = Math.min(a.y, b.y);
  const maxY = Math.max(a.y, b.y);
  if (maxX < r.x || minX > r.x + r.w || maxY < r.y || minY > r.y + r.h) {
    return false;
  }
  // Horizontal or vertical segment.
  if (Math.abs(a.y - b.y) < 0.5) {
    const y = a.y;
    return y >= r.y && y <= r.y + r.h && maxX >= r.x && minX <= r.x + r.w;
  }
  if (Math.abs(a.x - b.x) < 0.5) {
    const x = a.x;
    return x >= r.x && x <= r.x + r.w && maxY >= r.y && minY <= r.y + r.h;
  }
  return true;
}

function pathHitsObstacles(pts: Position[], obstacles: Rect[]): boolean {
  for (let i = 1; i < pts.length; i++) {
    const a = pts[i - 1]!;
    const b = pts[i]!;
    for (const o of obstacles) {
      if (segHitsRect(a, b, o)) return true;
    }
  }
  return false;
}

function pathLength(pts: Position[]): number {
  let n = 0;
  for (let i = 1; i < pts.length; i++) {
    n += Math.hypot(pts[i]!.x - pts[i - 1]!.x, pts[i]!.y - pts[i - 1]!.y);
  }
  return n;
}

function dedupeCorners(pts: Position[]): Position[] {
  const out: Position[] = [];
  for (const p of pts) {
    const last = out[out.length - 1];
    if (last && Math.abs(last.x - p.x) < 0.5 && Math.abs(last.y - p.y) < 0.5) {
      continue;
    }
    out.push({ x: snap(p.x), y: snap(p.y) });
  }
  return out;
}

/** Build candidate orthogonal polylines from start to end. */
function candidates(start: Position, end: Position, obstacles: Rect[]): Position[][] {
  const mx = snap((start.x + end.x) / 2);
  const my = snap((start.y + end.y) / 2);
  const feedback = end.x + 20 < start.x;
  const list: Position[][] = [];

  // Direct HV / VH / mid-X / mid-Y
  list.push([start, { x: end.x, y: start.y }, end]);
  list.push([start, { x: start.x, y: end.y }, end]);
  list.push([start, { x: mx, y: start.y }, { x: mx, y: end.y }, end]);
  list.push([start, { x: start.x, y: my }, { x: end.x, y: my }, end]);

  // Lane offsets around mid
  for (const dy of [-LANE * 2, -LANE, LANE, LANE * 2, LANE * 3]) {
    const y = snap(my + dy);
    list.push([start, { x: start.x, y }, { x: end.x, y }, end]);
  }
  for (const dx of [-LANE * 2, -LANE, LANE, LANE * 2]) {
    const x = snap(mx + dx);
    list.push([start, { x, y: start.y }, { x, y: end.y }, end]);
  }

  // Feedback / wrap-around: exit right, travel vertical outside, enter left
  if (feedback || Math.abs(end.x - start.x) < 40) {
    let top = Math.min(start.y, end.y) - LANE * 2;
    let bot = Math.max(start.y, end.y) + LANE * 2;
    for (const o of obstacles) {
      top = Math.min(top, o.y - LANE);
      bot = Math.max(bot, o.y + o.h + LANE);
    }
    const exitX = snap(start.x + LANE);
    const enterX = snap(end.x - LANE);
    list.push([
      start,
      { x: exitX, y: start.y },
      { x: exitX, y: snap(top) },
      { x: enterX, y: snap(top) },
      { x: enterX, y: end.y },
      end,
    ]);
    list.push([
      start,
      { x: exitX, y: start.y },
      { x: exitX, y: snap(bot) },
      { x: enterX, y: snap(bot) },
      { x: enterX, y: end.y },
      end,
    ]);
  }

  return list.map(dedupeCorners);
}

/**
 * Auto orthogonal route that prefers paths not cutting through node bodies.
 * Returns intermediate vertices (excluding endpoints) for DiagramEdge.vertices.
 */
export function autoRouteVertices(
  start: Position,
  end: Position,
  obstacles: Rect[],
): Position[] {
  const inflated = obstacles.map((o) => inflate(o, PAD));
  const cands = candidates(start, end, inflated);
  let best: Position[] | null = null;
  let bestLen = Infinity;
  let bestClear: Position[] | null = null;
  let bestClearLen = Infinity;

  for (const raw of cands) {
    const pts = orthogonalize(raw);
    const len = pathLength(pts);
    if (len < bestLen) {
      bestLen = len;
      best = pts;
    }
    if (!pathHitsObstacles(pts, inflated) && len < bestClearLen) {
      bestClearLen = len;
      bestClear = pts;
    }
  }

  const chosen = bestClear ?? best ?? [start, end];
  // Intermediate only
  if (chosen.length <= 2) return [];
  return chosen.slice(1, -1);
}

export function obstaclesForEdge(
  diagram: DiagramDocument,
  sourceId: string,
  targetId: string,
): Rect[] {
  return diagram.nodes
    .filter(
      (n) =>
        !isDrawNode(n) &&
        n.nodeKind !== "annotation" &&
        n.id !== sourceId &&
        n.id !== targetId,
    )
    .map(nodeRect);
}

/** Recompute vertices for one edge using current node layout. */
export function routeEdgeVertices(
  diagram: DiagramDocument,
  edge: { sourceNodeId: string; sourcePortId: string; targetNodeId: string; targetPortId: string },
): Position[] {
  const sn = diagram.nodes.find((n) => n.id === edge.sourceNodeId);
  const tn = diagram.nodes.find((n) => n.id === edge.targetNodeId);
  if (!sn || !tn) return [];
  const start = portAnchor(sn, edge.sourcePortId, "source");
  const end = portAnchor(tn, edge.targetPortId, "target");
  const obs = obstaclesForEdge(diagram, sn.id, tn.id);
  return autoRouteVertices(start, end, obs);
}

/** Re-route all edges (or only ids). */
export function rerouteDiagramEdges(
  diagram: DiagramDocument,
  edgeIds?: string[],
): DiagramDocument {
  const want = edgeIds ? new Set(edgeIds) : null;
  return {
    ...diagram,
    edges: diagram.edges.map((e) => {
      if (want && !want.has(e.id)) return e;
      return {
        ...e,
        vertices: routeEdgeVertices(diagram, e),
        routing: "orthogonal",
      };
    }),
  };
}

/** Re-route edges that start or end on the given nodes. */
export function rerouteEdgesTouchingNodes(
  diagram: DiagramDocument,
  nodeIds: Iterable<string>,
): DiagramDocument {
  const idSet = nodeIds instanceof Set ? nodeIds : new Set(nodeIds);
  if (!idSet.size) return diagram;
  const touched = diagram.edges
    .filter((e) => idSet.has(e.sourceNodeId) || idSet.has(e.targetNodeId))
    .map((e) => e.id);
  return touched.length ? rerouteDiagramEdges(diagram, touched) : diagram;
}
