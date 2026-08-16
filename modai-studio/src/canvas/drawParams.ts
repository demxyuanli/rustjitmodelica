import type { DiagramNode } from "../api/types";

export function isDrawNode(n: DiagramNode): boolean {
  return n.nodeKind === "draw" || n.domain === "draw" || n.catalogId.startsWith("draw.");
}

export function drawShape(n: DiagramNode): string {
  const t = n.typeName.split(".").pop() ?? "";
  return t.toLowerCase();
}

export function paramOf(n: DiagramNode, name: string, fallback = ""): string {
  return n.params.find((p) => p.name === name)?.value ?? fallback;
}

export function drawDefaultSize(catalogId: string): { width: number; height: number } {
  switch (catalogId) {
    case "draw.ellipse":
      return { width: 100, height: 80 };
    case "draw.line":
      return { width: 140, height: 16 };
    case "draw.polygon":
      return { width: 100, height: 80 };
    case "draw.text":
      return { width: 140, height: 28 };
    case "draw.note":
      return { width: 160, height: 72 };
    case "draw.rectangle":
    default:
      return { width: 140, height: 80 };
  }
}

export function dashArray(pattern: string, strokeWidth: number): string | undefined {
  const p = pattern.toLowerCase();
  if (p === "dashed") return `${6 * strokeWidth} ${4 * strokeWidth}`;
  if (p === "dotted") return `${strokeWidth} ${3 * strokeWidth}`;
  if (p === "dotdashed") return `${6 * strokeWidth} ${3 * strokeWidth} ${strokeWidth} ${3 * strokeWidth}`;
  return undefined;
}
