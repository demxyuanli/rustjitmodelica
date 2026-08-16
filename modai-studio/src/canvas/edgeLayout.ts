import type { DiagramDocument, DiagramEdge } from "../api/types";

/** Group key for fan-out from the same source port. */
export function branchKey(e: { sourceNodeId: string; sourcePortId: string }): string {
  return `${e.sourceNodeId}::${e.sourcePortId}`;
}

export function branchFanOut(diagram: DiagramDocument): Map<string, number> {
  const counts = new Map<string, number>();
  for (const e of diagram.edges) {
    const k = branchKey(e);
    counts.set(k, (counts.get(k) ?? 0) + 1);
  }
  return counts;
}

/** Prefer short label: "gain1.y" -> "y"; keep full if no dot. */
export function shortEdgeLabel(signalName: string | null | undefined): string | undefined {
  if (!signalName) return undefined;
  const t = signalName.trim();
  if (!t) return undefined;
  const i = t.lastIndexOf(".");
  if (i >= 0 && i < t.length - 1) return t.slice(i + 1);
  return t;
}

/**
 * Among edges sharing a branch, only the first (stable by id) shows the label
 * to avoid stacked text at the junction.
 */
export function shouldShowBranchLabel(
  e: DiagramEdge,
  fanOut: Map<string, number>,
  edges: DiagramEdge[],
): boolean {
  const k = branchKey(e);
  const n = fanOut.get(k) ?? 0;
  if (n <= 1) return true;
  const siblings = edges
    .filter((x) => branchKey(x) === k)
    .map((x) => x.id)
    .sort();
  return siblings[0] === e.id;
}

export function isBranchedEdge(
  e: { sourceNodeId: string; sourcePortId: string },
  fanOut: Map<string, number>,
): boolean {
  return (fanOut.get(branchKey(e)) ?? 0) > 1;
}
