import type { DiagramNode } from "../api/types";

export function isInportNode(n: DiagramNode): boolean {
  const id = n.catalogId.toLowerCase();
  return id.includes("inport") || id.includes("terminals.in");
}

export function isOutportNode(n: DiagramNode): boolean {
  const id = n.catalogId.toLowerCase();
  return id.includes("outport") || id.includes("terminals.out");
}

export function isTerminalNode(n: DiagramNode): boolean {
  return isInportNode(n) || isOutportNode(n);
}

export function terminalPortNumber(n: DiagramNode): string {
  const v = n.params.find((p) => p.name === "portNumber")?.value?.trim();
  return v && v.length > 0 ? v : "1";
}
