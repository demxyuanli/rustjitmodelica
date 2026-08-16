import type { DiagramNode } from "../api/types";

export function isBusBarNode(n: DiagramNode): boolean {
  const id = n.catalogId.toLowerCase();
  return (
    id.includes("routing.mux") ||
    id.includes("routing.demux") ||
    id.includes("routing.bus") ||
    id.includes(".mux") ||
    id.includes(".demux") ||
    id.includes(".bus")
  );
}

export function isMuxNode(n: DiagramNode): boolean {
  const id = n.catalogId.toLowerCase();
  return (id.includes("mux") || id.includes("bus")) && !id.includes("demux");
}

export function isDemuxNode(n: DiagramNode): boolean {
  return n.catalogId.toLowerCase().includes("demux");
}

export function isBusCreatorNode(n: DiagramNode): boolean {
  return n.catalogId.toLowerCase().includes("routing.bus");
}

export function busBarKind(n: DiagramNode): "mux" | "demux" | "bus" {
  if (isBusCreatorNode(n)) return "bus";
  if (isDemuxNode(n)) return "demux";
  return "mux";
}

export function busBarHeight(portCount: number): number {
  return Math.max(48, 16 + portCount * 18);
}

/** Face badge like "3→1" for Mux / "1→3" for Demux. */
export function busDimensionBadge(leftCount: number, rightCount: number): string {
  return `${Math.max(leftCount, 0)}\u2192${Math.max(rightCount, 0)}`;
}
