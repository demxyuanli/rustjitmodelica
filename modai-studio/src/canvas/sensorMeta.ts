import type { DiagramNode } from "../api/types";

/**
 * F71 bridge convention: Modelica sensors already expose RealOutput ports.
 * No separate PS-Simulink converter block — connect sensor.*.out to signal inputs.
 *
 * Default SI unit annotations for common sensor / source ports (display only).
 */
const PORT_UNITS: Record<string, string> = {
  // electrical
  v: "V",
  i: "A",
  phi: "V",
  power: "W",
  // rotational
  w: "rad/s",
  a: "rad/s2",
  tau: "N.m",
  phi_rel: "rad",
  w_rel: "rad/s",
  // translational (port `a` shared with rotational — catalog disambiguates below)
  s: "m",
  f: "N",
  v_ref: "m/s",
  s_ref: "m",
  s_rel: "m",
  v_rel: "m/s",
  f_rel: "N",
  w_ref: "rad/s",
};

/** Infer unit for a node/port (prefer source sensor ports). */
export function inferPortUnit(node: DiagramNode, portId: string): string | null {
  const id = (node.catalogId ?? "").toLowerCase();
  const p = portId.toLowerCase();

  if (id.includes("voltagesensor") && (p === "v" || p === "y")) return "V";
  if (id.includes("currentsensor") && (p === "i" || p === "y")) return "A";
  if (id.includes("potentialsensor") && p === "phi") return "V";
  if (id.includes("powersensor") && p === "power") return "W";

  if (id.includes("rotational") || id.includes("anglesensor") || id.includes("torquesensor")) {
    if (p === "phi" || p === "phi_rel") return "rad";
    if (p === "w" || p === "w_rel" || p === "w_ref") return "rad/s";
    if (p === "a") return "rad/s2";
    if (p === "tau") return "N.m";
  }
  if (id.includes("translational") || id.includes("positionsensor") || id.includes("forcesensor")) {
    if (p === "s" || p === "s_rel" || p === "s_ref") return "m";
    if (p === "v" || p === "v_rel" || p === "v_ref") return "m/s";
    if (p === "a") return "m/s2";
    if (p === "f" || p === "f_rel") return "N";
  }

  return PORT_UNITS[p] ?? null;
}

/** Dimension hint for vector-ish ports; scalars default to "1". */
export function inferPortDimension(node: DiagramNode, portId: string): string | null {
  const port = node.ports.find((x) => x.id === portId);
  if (!port) return null;
  if (port.kind !== "signal") return null;
  return "1";
}
