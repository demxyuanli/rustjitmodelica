import type { DiagramDocument, DiagramEdge, DiagramNode } from "../api/types";

/** Flatten Modelica-style names: `body.r_0[1]` -> `body_r_0_1`. */
export function normalizeResultKey(name: string): string {
  return name
    .trim()
    .replace(/\[(\d+)\]/g, "_$1")
    .replace(/\./g, "_");
}

/** True when a compiler series key is the same variable as a plot-hint / UI name. */
export function seriesKeyMatchesHint(seriesKey: string, hint: string): boolean {
  if (!seriesKey || !hint || seriesKey === "time") return false;
  if (seriesKey === hint) return true;
  const nk = normalizeResultKey(seriesKey);
  const nh = normalizeResultKey(hint);
  if (nk === nh) return true;
  if (nk.endsWith(`_${nh}`)) return true;
  return false;
}

/** Resolve UI/hint names onto actual `result.series` keys, preserving hint order. */
export function resolveSeriesKeys(seriesKeys: string[], names: string[]): string[] {
  const out: string[] = [];
  for (const name of names) {
    const hit = seriesKeys.find((k) => seriesKeyMatchesHint(k, name));
    if (hit && !out.includes(hit)) out.push(hit);
  }
  return out;
}

/** Build preferred result variable names from diagram node plotHints. */
export function collectPlotHintVars(diagram: DiagramDocument): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  for (const n of diagram.nodes) {
    for (const h of n.plotHints ?? []) {
      const hint = h.trim();
      if (!hint) continue;
      const key = `${n.instanceName}.${hint}`;
      if (seen.has(key)) continue;
      seen.add(key);
      out.push(key);
    }
  }
  return out;
}

/** Pick series keys that match preferred plot-hint names. */
export function pickSeriesByHints(seriesKeys: string[], hints: string[]): string[] {
  const plotKeys = seriesKeys.filter((k) => k !== "time");
  if (!plotKeys.length) return [];
  if (!hints.length) return plotKeys.slice(0, 3);
  const picked = resolveSeriesKeys(plotKeys, hints);
  return picked.length ? picked.slice(0, 8) : plotKeys.slice(0, 3);
}

function matchSeries(seriesKeys: string[], candidates: string[]): string[] {
  return resolveSeriesKeys(
    seriesKeys.filter((k) => k !== "time"),
    candidates,
  );
}

/** F51: map a selected edge to result series keys. */
export function seriesKeysForEdge(
  diagram: DiagramDocument,
  edge: DiagramEdge,
  seriesKeys: string[],
): string[] {
  const src = diagram.nodes.find((n) => n.id === edge.sourceNodeId);
  const candidates: string[] = [];
  if (edge.signalName?.trim()) candidates.push(edge.signalName.trim());
  if (src) {
    candidates.push(`${src.instanceName}.${edge.sourcePortId}`);
    candidates.push(`${src.instanceName}.y`);
  }
  return matchSeries(seriesKeys, candidates);
}

/** F50: Scope / Display node plotHints → series. */
export function seriesKeysForSinkNode(node: DiagramNode, seriesKeys: string[]): string[] {
  const hints = (node.plotHints ?? []).map((h) => `${node.instanceName}.${h}`);
  hints.push(`${node.instanceName}.y`, `${node.instanceName}.u`);
  return matchSeries(seriesKeys, hints);
}

/** F52: last sample for Display blocks. */
export function displayEndValues(
  diagram: DiagramDocument,
  series: Record<string, number[]>,
): Record<string, string> {
  const keys = Object.keys(series);
  const out: Record<string, string> = {};
  for (const n of diagram.nodes) {
    const id = (n.catalogId ?? "").toLowerCase();
    if (!id.includes("sinks.display") && !id.includes("sinks.scope")) continue;
    const matched = seriesKeysForSinkNode(n, keys);
    const k = matched[0];
    if (!k) continue;
    const arr = series[k];
    if (!arr?.length) continue;
    const v = arr[arr.length - 1]!;
    out[n.id] = Number.isFinite(v) ? v.toPrecision(4) : String(v);
  }
  return out;
}
