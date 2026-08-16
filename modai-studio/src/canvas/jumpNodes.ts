import type { DiagramNode } from "../api/types";

export function isGotoNode(n: DiagramNode): boolean {
  const id = n.catalogId.toLowerCase();
  return id.includes("routing.goto") || id.endsWith(".goto");
}

export function isFromNode(n: DiagramNode): boolean {
  const id = n.catalogId.toLowerCase();
  return id.includes("routing.from") || id.endsWith(".from");
}

export function isJumpNode(n: DiagramNode): boolean {
  return isGotoNode(n) || isFromNode(n);
}

export function jumpTag(n: DiagramNode): string {
  return n.params.find((p) => p.name === "tag")?.value?.trim() || "A";
}
