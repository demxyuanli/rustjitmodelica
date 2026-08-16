import type { DiagramDocument, DiagramNode, SimulationResult } from "../api/types";

export type Vec3 = [number, number, number];

export interface BodyPose {
  id: string;
  label: string;
  size: Vec3;
  shape: "box" | "cylinder" | "sphere";
  color: string;
  positions: Vec3[];
  eulers?: Vec3[];
}

export interface MultibodyScene {
  time: number[];
  bodies: BodyPose[];
  source: "simulation" | "diagram" | "empty";
}

const PALETTE = ["#5b9bd5", "#c47a3a", "#3dbf7a", "#e35d6a", "#b57edc", "#e6b800"];

function colorAt(i: number): string {
  return PALETTE[i % PALETTE.length]!;
}

function parseVec3Param(raw: string | undefined, fallback: Vec3): Vec3 {
  if (!raw) return fallback;
  const s = raw.trim();
  const m = s.match(/\{\s*([^}]+)\s*\}/);
  const inner = m ? m[1]! : s;
  const parts = inner.split(",").map((x) => Number(x.trim()));
  if (parts.length >= 3 && parts.every((n) => Number.isFinite(n))) {
    return [parts[0]!, parts[1]!, parts[2]!];
  }
  return fallback;
}

function paramOf(n: DiagramNode, name: string): string | undefined {
  return n.params.find((p) => p.name === name)?.value;
}

export function isMultibodyNode(n: DiagramNode): boolean {
  return (
    n.domain === "multibody" ||
    n.typeName.includes("MultiBody") ||
    n.catalogId.startsWith("multibody.")
  );
}

function findComponentKeys(
  keys: string[],
  component: string,
  field: string,
): { x?: string; y?: string; z?: string } {
  const esc = component.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const fieldEsc = field.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const out: { x?: string; y?: string; z?: string } = {};
  for (const k of keys) {
    if (
      new RegExp(`^${esc}\\.${fieldEsc}\\[1\\]$`).test(k) ||
      new RegExp(`^${esc}\\.${fieldEsc}_1$`).test(k) ||
      new RegExp(`^${esc}\\.frame_a\\.${fieldEsc}\\[1\\]$`).test(k) ||
      new RegExp(`^${esc}\\.frame_b\\.${fieldEsc}\\[1\\]$`).test(k)
    ) {
      out.x = k;
    } else if (
      new RegExp(`^${esc}\\.${fieldEsc}\\[2\\]$`).test(k) ||
      new RegExp(`^${esc}\\.${fieldEsc}_2$`).test(k) ||
      new RegExp(`^${esc}\\.frame_a\\.${fieldEsc}\\[2\\]$`).test(k) ||
      new RegExp(`^${esc}\\.frame_b\\.${fieldEsc}\\[2\\]$`).test(k)
    ) {
      out.y = k;
    } else if (
      new RegExp(`^${esc}\\.${fieldEsc}\\[3\\]$`).test(k) ||
      new RegExp(`^${esc}\\.${fieldEsc}_3$`).test(k) ||
      new RegExp(`^${esc}\\.frame_a\\.${fieldEsc}\\[3\\]$`).test(k) ||
      new RegExp(`^${esc}\\.frame_b\\.${fieldEsc}\\[3\\]$`).test(k)
    ) {
      out.z = k;
    }
  }
  return out;
}

function seriesLen(result: SimulationResult, keys: (string | undefined)[]): number {
  let n = result.time.length;
  for (const k of keys) {
    if (!k) continue;
    const s = result.series[k];
    if (s) n = Math.min(n, s.length);
  }
  return n;
}

function bodySizeFromNode(n: DiagramNode): { size: Vec3; shape: BodyPose["shape"] } {
  const tn = n.typeName;
  if (tn.includes("BodyCylinder") || tn.endsWith(".Cylinder")) {
    const d = Number(paramOf(n, "diameter") ?? "0.1") || 0.1;
    const L = Number(paramOf(n, "length") ?? "1") || 1;
    return { size: [d / 2, L, d / 2], shape: "cylinder" };
  }
  if (tn.includes("BodySphere")) {
    const d = Number(paramOf(n, "diameter") ?? "0.2") || 0.2;
    return { size: [d / 2, d / 2, d / 2], shape: "sphere" };
  }
  if (tn.includes("BodyBox")) {
    const L = Number(paramOf(n, "length") ?? "1") || 1;
    const W = Number(paramOf(n, "width") ?? "0.1") || 0.1;
    const H = Number(paramOf(n, "height") ?? "0.1") || 0.1;
    return { size: [L, H, W], shape: "box" };
  }
  if (tn.includes("FixedTranslation")) {
    const r = parseVec3Param(paramOf(n, "r"), [1, 0, 0]);
    const len = Math.hypot(r[0], r[1], r[2]) || 0.5;
    return { size: [len, 0.04, 0.04], shape: "box" };
  }
  return { size: [0.2, 0.2, 0.2], shape: "box" };
}

export function sceneFromSimulation(
  result: SimulationResult,
  diagram: DiagramDocument | null,
): MultibodyScene | null {
  const keys = Object.keys(result.series);
  const candidates = new Set<string>();
  for (const k of keys) {
    const m =
      k.match(/^(.+?)\.(?:frame_[ab]\.)?r_0(?:\[|_)/) || k.match(/^(.+?)_r_0_/);
    if (m?.[1] && !m[1].includes("[")) candidates.add(m[1]);
  }
  if (diagram) {
    for (const n of diagram.nodes) {
      if (isMultibodyNode(n) && /Body|Box|Cylinder|Sphere|FixedTranslation/i.test(n.typeName)) {
        candidates.add(n.instanceName);
      }
    }
  }
  if (!candidates.size) return null;

  const bodies: BodyPose[] = [];
  let bi = 0;
  for (const name of [...candidates].sort()) {
    const xyz = findComponentKeys(keys, name, "r_0");
    if (!xyz.x && !xyz.y && !xyz.z) continue;
    const nSamples = seriesLen(result, [xyz.x, xyz.y, xyz.z]);
    if (nSamples < 1) continue;
    const node = diagram?.nodes.find((d) => d.instanceName === name);
    const { size, shape } = node
      ? bodySizeFromNode(node)
      : { size: [0.2, 0.2, 0.2] as Vec3, shape: "box" as const };
    const positions: Vec3[] = [];
    for (let i = 0; i < nSamples; i++) {
      positions.push([
        xyz.x ? (result.series[xyz.x]?.[i] ?? 0) : 0,
        xyz.y ? (result.series[xyz.y]?.[i] ?? 0) : 0,
        xyz.z ? (result.series[xyz.z]?.[i] ?? 0) : 0,
      ]);
    }
    bodies.push({
      id: name,
      label: name,
      size,
      shape,
      color: colorAt(bi++),
      positions,
    });
  }
  if (!bodies.length) return null;
  const len = Math.min(...bodies.map((b) => b.positions.length), result.time.length);
  return {
    time: result.time.slice(0, len),
    bodies: bodies.map((b) => ({ ...b, positions: b.positions.slice(0, len) })),
    source: "simulation",
  };
}

function tipPose(from: DiagramNode, to: DiagramNode, fromPos: Vec3): Vec3 {
  const mover = to.typeName.includes("FixedTranslation")
    ? to
    : from.typeName.includes("FixedTranslation")
      ? from
      : null;
  if (mover) {
    const r = parseVec3Param(paramOf(mover, "r"), [0.5, 0, 0]);
    if (mover.id === to.id) {
      return [fromPos[0] + r[0], fromPos[1] + r[1], fromPos[2] + r[2]];
    }
    return [fromPos[0] - r[0], fromPos[1] - r[1], fromPos[2] - r[2]];
  }
  const rCm = parseVec3Param(paramOf(to, "r_CM"), [0, 0, 0]);
  return [fromPos[0] + rCm[0], fromPos[1] + rCm[1], fromPos[2] + rCm[2]];
}

export function sceneFromDiagram(diagram: DiagramDocument): MultibodyScene {
  const nodes = diagram.nodes.filter(isMultibodyNode);
  if (!nodes.length) {
    return { time: [0], bodies: [], source: "empty" };
  }

  const byId = new Map(nodes.map((n) => [n.id, n]));
  const world = nodes.find((n) => n.typeName.includes("World"));
  const positions = new Map<string, Vec3>();
  if (world) positions.set(world.id, [0, 0, 0]);

  for (let iter = 0; iter < nodes.length + 2; iter++) {
    for (const e of diagram.edges) {
      const a = byId.get(e.sourceNodeId);
      const b = byId.get(e.targetNodeId);
      if (!a || !b) continue;
      const pa = positions.get(a.id);
      const pb = positions.get(b.id);
      if (pa && !pb) positions.set(b.id, tipPose(a, b, pa));
      else if (pb && !pa) positions.set(a.id, tipPose(b, a, pb));
    }
  }

  for (const n of nodes) {
    if (positions.has(n.id)) continue;
    if (n.typeName.includes("World")) {
      positions.set(n.id, [0, 0, 0]);
      continue;
    }
    let r = parseVec3Param(paramOf(n, "r_0"), [NaN, NaN, NaN]);
    if (!Number.isFinite(r[0])) r = parseVec3Param(paramOf(n, "r_CM"), [NaN, NaN, NaN]);
    if (!Number.isFinite(r[0])) r = parseVec3Param(paramOf(n, "r"), [0, 0, 0]);
    positions.set(n.id, r);
  }

  const bodies: BodyPose[] = [];
  let bi = 0;
  if (world) {
    bodies.push({
      id: world.id,
      label: world.instanceName,
      size: [0.35, 0.02, 0.35],
      shape: "box",
      color: "#6a737d",
      positions: [[0, 0, 0]],
    });
  }

  for (const n of nodes) {
    if (n.typeName.includes("World")) continue;
    if (n.typeName.includes("Revolute") || n.typeName.includes("Joint")) continue;
    if (n.typeName.includes(".Fixed") && !n.typeName.includes("FixedTranslation")) continue;
    const pos = positions.get(n.id) ?? [0, 0, 0];
    const { size, shape } = bodySizeFromNode(n);
    bodies.push({
      id: n.id,
      label: n.instanceName,
      size,
      shape,
      color: colorAt(bi++),
      positions: [pos],
    });
  }

  return { time: [0], bodies, source: bodies.length ? "diagram" : "empty" };
}

export function buildMultibodyScene(
  diagram: DiagramDocument | null,
  result: SimulationResult | null,
): MultibodyScene {
  if (result) {
    const sim = sceneFromSimulation(result, diagram);
    if (sim) return sim;
  }
  if (diagram) return sceneFromDiagram(diagram);
  return { time: [0], bodies: [], source: "empty" };
}

export function diagramHasMultibody(diagram: DiagramDocument | null): boolean {
  return !!diagram?.nodes.some(isMultibodyNode);
}
