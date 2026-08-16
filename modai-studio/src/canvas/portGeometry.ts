import type { DiagramNode, PortDef, Position } from "../api/types";

export type PortSide = "left" | "right" | "top" | "bottom";
export type Rot90 = 0 | 90 | 180 | 270;

export type SidePort = {
  id: string;
  type: "source" | "target";
  kind: string;
  label: string;
  tip: string;
};

const CLOCKWISE: PortSide[] = ["left", "top", "right", "bottom"];

export function normalizeRotation(deg?: number): Rot90 {
  const r = (((Math.round((deg ?? 0) / 90) * 90) % 360) + 360) % 360;
  if (r === 90 || r === 180 || r === 270) return r;
  return 0;
}

export function rotateSide(side: PortSide, rot: Rot90): PortSide {
  if (rot === 0) return side;
  const i = CLOCKWISE.indexOf(side);
  return CLOCKWISE[(i + rot / 90) % 4]!;
}

export function nodeBoxSize(n: DiagramNode): { w: number; h: number } {
  const w = n.ui?.width ?? (n.nodeKind === "subsystem" ? 140 : 120);
  const h = n.ui?.height ?? 64;
  const rot = normalizeRotation(n.rotation);
  if (rot === 90 || rot === 270) return { w: h, h: w };
  return { w, h };
}

function portLabel(p: PortDef): string {
  return p.id === "self" ? "" : p.id;
}

/** Unrotated layout: ins/connectors on the left, outs on the right. */
export function layoutPorts(n: DiagramNode): { left: SidePort[]; right: SidePort[] } {
  const signalIns = n.ports.filter((p) => p.kind === "signal" && p.direction === "in");
  const signalOuts = n.ports.filter((p) => p.kind === "signal" && p.direction === "out");
  const cons = n.ports.filter((p) => p.kind === "connector");

  const left: SidePort[] = signalIns.map((p) => ({
    id: p.id,
    type: "target" as const,
    kind: p.kind,
    label: portLabel(p),
    tip: `${p.id} · ${p.kind} ${p.direction} · ${p.typeName}`,
  }));
  const right: SidePort[] = signalOuts.map((p) => ({
    id: p.id,
    type: "source" as const,
    kind: p.kind,
    label: portLabel(p),
    tip: `${p.id} · ${p.kind} ${p.direction} · ${p.typeName}`,
  }));

  if (cons.length === 1) {
    left.push({
      id: cons[0]!.id,
      type: "target",
      kind: "connector",
      label: portLabel(cons[0]!),
      tip: `${cons[0]!.id} · connector · ${cons[0]!.typeName}`,
    });
  } else if (cons.length > 1) {
    for (const p of cons.slice(0, -1)) {
      left.push({
        id: p.id,
        type: "target",
        kind: "connector",
        label: portLabel(p),
        tip: `${p.id} · connector · ${p.typeName}`,
      });
    }
    const last = cons[cons.length - 1]!;
    right.push({
      id: last.id,
      type: "source",
      kind: "connector",
      label: portLabel(last),
      tip: `${last.id} · connector · ${last.typeName}`,
    });
  }

  return { left, right };
}

export function portsByRotatedSide(n: DiagramNode): Record<PortSide, SidePort[]> {
  const { left, right } = layoutPorts(n);
  const rot = normalizeRotation(n.rotation);
  const out: Record<PortSide, SidePort[]> = { left: [], right: [], top: [], bottom: [] };
  for (const p of left) out[rotateSide("left", rot)].push(p);
  for (const p of right) out[rotateSide("right", rot)].push(p);
  return out;
}

export function portAnchorOnBox(
  n: DiagramNode,
  portId: string,
  role: "source" | "target",
  box: { x: number; y: number; w: number; h: number },
): Position {
  const sides = portsByRotatedSide(n);
  let side: PortSide = role === "source" ? "right" : "left";
  let idx = 0;
  let list: SidePort[] = [];
  for (const s of CLOCKWISE) {
    const found = sides[s].findIndex((p) => p.id === portId);
    if (found >= 0) {
      side = s;
      idx = found;
      list = sides[s];
      break;
    }
  }
  if (!list.length) {
    list = role === "source" ? sides.right : sides.left;
    side = role === "source" ? "right" : "left";
  }
  const frac = (idx + 1) / (list.length + 1);
  if (side === "left") return { x: box.x, y: box.y + box.h * frac };
  if (side === "right") return { x: box.x + box.w, y: box.y + box.h * frac };
  if (side === "top") return { x: box.x + box.w * frac, y: box.y };
  return { x: box.x + box.w * frac, y: box.y + box.h };
}
