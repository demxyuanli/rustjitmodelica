import type { SimulationResult } from "../api/types";

export function exportResultsCsv(result: SimulationResult, pathHint?: string): void {
  const keys = Object.keys(result.series).sort();
  const lines = ["time," + keys.join(",")];
  for (let i = 0; i < result.time.length; i++) {
    const row = [String(result.time[i]), ...keys.map((k) => String(result.series[k]?.[i] ?? ""))];
    lines.push(row.join(","));
  }
  const blob = new Blob([lines.join("\n")], { type: "text/csv;charset=utf-8" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = pathHint ?? "studio_results.csv";
  a.click();
  URL.revokeObjectURL(a.href);
}

export function exportResultsJson(result: SimulationResult, pathHint?: string): void {
  const blob = new Blob([JSON.stringify(result, null, 2)], { type: "application/json" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = pathHint ?? "studio_results.json";
  a.click();
  URL.revokeObjectURL(a.href);
}
