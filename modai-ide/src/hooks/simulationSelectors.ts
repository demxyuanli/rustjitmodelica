import type { JitValidateResult, SimulationResult } from "../types";
import type { SimulationChartMeta, SimulationChartSeries } from "../components/simulation/types";

export function buildSimulationTableColumns(simResult: SimulationResult | null): string[] {
  if (!simResult) return [];
  return ["time", ...Object.keys(simResult.series).filter((key) => key !== "time")];
}

export function buildSimulationTableRows(simResult: SimulationResult | null): Record<string, number>[] {
  if (!simResult) return [];
  return simResult.time.map((_, index) => {
    const row: Record<string, number> = { time: simResult.time[index] };
    for (const key of Object.keys(simResult.series)) {
      row[key] = simResult.series[key][index];
    }
    return row;
  });
}

export function sortSimulationTableRows(
  rows: Record<string, number>[],
  sortKey: string,
  sortAsc: boolean,
): Record<string, number>[] {
  return [...rows].sort((left, right) => {
    const leftValue = left[sortKey];
    const rightValue = right[sortKey];
    if (leftValue == null || rightValue == null) return 0;
    const compare = leftValue < rightValue ? -1 : leftValue > rightValue ? 1 : 0;
    return sortAsc ? compare : -compare;
  });
}

export function buildSimulationChartSeries(
  simResult: SimulationResult | null,
  selectedPlotVars: string[],
): SimulationChartSeries[] {
  if (!simResult) return [];
  return selectedPlotVars
    .filter((name) => simResult.series[name] != null)
    .map((name) => ({
      name,
      values: simResult.series[name],
    }));
}

export function buildSimulationChartMeta(
  simResult: SimulationResult | null,
  series: SimulationChartSeries[],
): SimulationChartMeta {
  const time = simResult?.time ?? [];
  return {
    pointCount: time.length,
    seriesCount: series.length,
    xMin: time.length > 0 ? time[0] : null,
    xMax: time.length > 0 ? time[time.length - 1] : null,
  };
}

export function listSimulationPlotVarNames(
  simResult: SimulationResult | null,
  jitResult: JitValidateResult | null,
): string[] {
  if (simResult) {
    return Object.keys(simResult.series).filter((key) => key !== "time");
  }
  if (jitResult) {
    return [...new Set([...(jitResult.state_vars ?? []), ...(jitResult.output_vars ?? [])])];
  }
  return [];
}
