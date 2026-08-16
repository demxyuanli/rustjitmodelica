import type { DiagramDocument, DiagramEdge, DiagramNode } from "../api/types";
import { findNode, portOf } from "./connectionRules";

export type EdgeDomain = "signal" | "electrical" | "mechanical" | "multibody" | "generic";

export type EdgeVisual = {
  domain: EdgeDomain;
  stroke: string;
  strokeWidth: number;
  strokeDasharray?: string;
  marker: boolean;
};

/** Infer connection domain from source node / port type (F72). */
export function edgeDomainOf(diagram: DiagramDocument, edge: DiagramEdge): EdgeDomain {
  const src = findNode(diagram, edge.sourceNodeId);
  if (!src) return "generic";
  const d = src.domain;
  if (d === "signal" || d === "electrical" || d === "mechanical" || d === "multibody") {
    const port = portOf(src, edge.sourcePortId);
    if (port?.kind === "connector") {
      return domainFromPortType(port.typeName) ?? (d === "signal" ? "electrical" : d);
    }
    if (port?.kind === "signal") return "signal";
    return d;
  }
  const port = portOf(src, edge.sourcePortId);
  if (port) {
    const fromType = domainFromPortType(port.typeName);
    if (fromType) return fromType;
    if (port.kind === "signal") return "signal";
  }
  return "generic";
}

function domainFromPortType(typeName: string): EdgeDomain | null {
  const t = typeName.toLowerCase();
  if (t.includes("pin") || t.includes("electrical")) return "electrical";
  if (t.includes("multibody") || t.includes("frame_")) return "multibody";
  if (t.includes("rotational") || t.includes("translational") || t.includes("flange")) {
    return "mechanical";
  }
  if (t.includes("realinput") || t.includes("realoutput") || t.includes("boolean")) return "signal";
  return null;
}

export function edgeVisual(domain: EdgeDomain, bad: boolean): EdgeVisual {
  if (bad) {
    return {
      domain,
      stroke: "var(--danger)",
      strokeWidth: 2.5,
      marker: true,
    };
  }
  switch (domain) {
    case "signal":
      return { domain, stroke: "var(--signal)", strokeWidth: 1.75, marker: true };
    case "electrical":
      return { domain, stroke: "var(--connector)", strokeWidth: 2.85, marker: false };
    case "mechanical":
      return {
        domain,
        stroke: "var(--ok)",
        strokeWidth: 2.5,
        strokeDasharray: "7 3",
        marker: false,
      };
    case "multibody":
      return {
        domain,
        stroke: "color-mix(in srgb, var(--ok) 70%, var(--accent))",
        strokeWidth: 3,
        strokeDasharray: "3 2",
        marker: false,
      };
    default:
      return { domain, stroke: "var(--muted)", strokeWidth: 1.5, marker: true };
  }
}

/** Label text: name plus optional unit, e.g. `y [V]`. */
export function edgeDisplayLabel(
  signalName: string | null | undefined,
  unit: string | null | undefined,
  shortFn: (full: string | undefined) => string | undefined,
): string | undefined {
  const base = shortFn(signalName ?? undefined) ?? signalName ?? undefined;
  if (!base) {
    const u = unit?.trim();
    return u ? `[${u}]` : undefined;
  }
  const u = unit?.trim();
  return u ? `${base} [${u}]` : base;
}

export function edgeTooltip(
  edge: DiagramEdge,
  domain: EdgeDomain,
  src: DiagramNode | undefined,
): string {
  const parts = [
    edge.signalName || `${src?.instanceName ?? "?"}.${edge.sourcePortId}`,
    `domain=${domain}`,
  ];
  if (edge.unit?.trim()) parts.push(`unit=${edge.unit.trim()}`);
  if (edge.dimension?.trim()) parts.push(`dim=${edge.dimension.trim()}`);
  return parts.join(" · ");
}
