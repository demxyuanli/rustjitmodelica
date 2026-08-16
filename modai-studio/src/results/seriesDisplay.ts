import type { DiagramNode } from "../api/types";
import { normalizeResultKey } from "../app/plotHints";

export interface SeriesDisplay {
  key: string;
  label: string;
  instance: string;
  rest: string;
}

function prettyRest(after: string): string {
  return after.replace(/_(\d+)$/g, "[$1]");
}

export function displaySeries(key: string, instanceNames: string[]): SeriesDisplay {
  if (key === "time") {
    return { key, label: "time", instance: "", rest: "time" };
  }
  const nk = normalizeResultKey(key);
  const sorted = [...instanceNames].sort((a, b) => b.length - a.length);
  for (const inst of sorted) {
    const prefix = normalizeResultKey(inst);
    if (!prefix) continue;
    if (nk === prefix) {
      return { key, label: inst, instance: inst, rest: "" };
    }
    if (nk.startsWith(`${prefix}_`)) {
      const rest = prettyRest(nk.slice(prefix.length + 1));
      return { key, label: rest ? `${inst}.${rest}` : inst, instance: inst, rest };
    }
  }
  return { key, label: key, instance: "", rest: key };
}

export function instanceNamesFromNodes(nodes: DiagramNode[]): string[] {
  return nodes
    .filter((n) => n.nodeKind !== "draw" && n.nodeKind !== "annotation")
    .map((n) => n.instanceName)
    .filter(Boolean);
}

export interface SeriesGroup {
  instance: string;
  items: SeriesDisplay[];
}

export function groupSeries(keys: string[], nodes: DiagramNode[]): SeriesGroup[] {
  const names = instanceNamesFromNodes(nodes);
  const items = keys.filter((k) => k !== "time").map((k) => displaySeries(k, names));
  const by = new Map<string, SeriesDisplay[]>();
  for (const it of items) {
    const g = it.instance || "";
    const list = by.get(g) ?? [];
    list.push(it);
    by.set(g, list);
  }
  const groups: SeriesGroup[] = [];
  for (const [instance, list] of by) {
    list.sort((a, b) => a.label.localeCompare(b.label));
    groups.push({ instance, items: list });
  }
  groups.sort((a, b) => {
    if (!a.instance) return 1;
    if (!b.instance) return -1;
    return a.instance.localeCompare(b.instance);
  });
  return groups;
}

export function collectParamSnapshot(
  nodes: DiagramNode[],
  dictionary: { name: string; value: string }[],
): Record<string, string> {
  const out: Record<string, string> = {};
  for (const n of nodes) {
    if (n.nodeKind === "draw" || n.nodeKind === "annotation") continue;
    for (const p of n.params) {
      out[`${n.instanceName}.${p.name}`] = p.value;
    }
  }
  for (const d of dictionary) {
    out[`$${d.name}`] = d.value;
  }
  return out;
}

export function mergeLiveSeries(
  prev: { time: number[]; series: Record<string, number[]> } | null,
  liveTime: number[],
  liveSeries: Record<string, number[]>,
): { time: number[]; series: Record<string, number[]> } {
  const time = [...(prev?.time ?? []), ...liveTime];
  const series: Record<string, number[]> = { ...(prev?.series ?? {}) };
  for (const [k, vals] of Object.entries(liveSeries)) {
    series[k] = [...(series[k] ?? []), ...vals];
  }
  series.time = time;
  return { time, series };
}

export function diffSnapshots(
  curr: Record<string, string> | undefined,
  prev: Record<string, string> | undefined,
): { key: string; from: string; to: string }[] {
  if (!curr) return [];
  const keys = new Set([...Object.keys(curr), ...Object.keys(prev ?? {})]);
  const out: { key: string; from: string; to: string }[] = [];
  for (const k of [...keys].sort()) {
    const a = prev?.[k];
    const b = curr[k];
    if (a === b) continue;
    if (a == null && b == null) continue;
    out.push({ key: k, from: a ?? "", to: b ?? "" });
  }
  return out;
}
