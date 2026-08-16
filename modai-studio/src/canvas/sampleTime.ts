import type { DiagramNode } from "../api/types";

/** Infer sample-time class for F73 coloring. */
export function sampleTimeClass(n: DiagramNode): "continuous" | "discrete" | "inherit" | "none" {
  if (n.nodeKind === "draw" || n.nodeKind === "annotation") return "none";
  const id = n.catalogId.toLowerCase();
  if (
    id.includes("discrete") ||
    id.includes("unitdelay") ||
    id.includes("zeroorder") ||
    id.includes("sampler") ||
    id.includes("ratetransition") ||
    id.includes("stategraph")
  ) {
    return "discrete";
  }
  if (n.params.some((p) => p.name === "samplePeriod")) return "discrete";
  if (n.domain === "signal" || n.domain === "electrical" || n.domain === "multibody") {
    if (n.nodeKind === "subsystem" || n.nodeKind === "fmu") return "inherit";
    if (id.includes("continuous") || id.includes("integrator") || id.includes("transferfunction")) {
      return "continuous";
    }
    if (n.domain === "signal") return "continuous";
  }
  return "inherit";
}

export function samplePeriodOf(n: DiagramNode): number | null {
  const p = n.params.find((x) => x.name === "samplePeriod");
  if (!p) return null;
  const v = Number.parseFloat(String(p.value).trim());
  return Number.isFinite(v) && v > 0 ? v : null;
}

/** Explicit Rate Transition catalog entries (excuse Ts mismatch). */
export function isRateTransitionNode(n: DiagramNode): boolean {
  const id = n.catalogId.toLowerCase();
  return id.includes("ratetransition");
}

export function samplePeriodLabel(n: DiagramNode): string | null {
  const p = n.params.find((x) => x.name === "samplePeriod");
  if (!p) return null;
  const id = n.catalogId.toLowerCase();
  if (id.includes("ratetransition")) return `RT Ts=${p.value}`;
  return `Ts=${p.value}`;
}
