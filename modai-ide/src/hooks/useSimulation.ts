import { useState, useCallback, useEffect, useMemo } from "react";
import type { JitValidateOptions, JitValidateResult, SimulationResult } from "../types";
import { jitValidate, runSimulation as runSimulationApi, readProjectFile } from "../api/tauri";
import { t } from "../i18n";

export interface SimulationParams {
  tEnd: number;
  dt: number;
  solver: string;
  outputInterval: number;
  atol: number;
  rtol: number;
}

export interface TableState {
  simViewMode: "chart" | "table";
  tableSortKey: string;
  tableSortAsc: boolean;
  tablePage: number;
  tablePageSize: number;
  visibleTableColumns: string[];
}

export function useSimulation(log: (msg: string) => void) {
  const [params, setParamsState] = useState<SimulationParams>({
    tEnd: 2, dt: 0.01, solver: "rk45", outputInterval: 0.05, atol: 1e-6, rtol: 1e-3,
  });
  const [jitResult, setJitResult] = useState<JitValidateResult | null>(null);
  const [simResult, setSimResult] = useState<SimulationResult | null>(null);
  const [selectedPlotVars, setSelectedPlotVars] = useState<string[]>([]);
  const [simLoading, setSimLoading] = useState(false);

  const [testAllLoading, setTestAllLoading] = useState(false);
  const [testAllResults, setTestAllResults] = useState<{ path: string; success: boolean; errors: string[] }[] | null>(null);

  const [tableState, setTableState] = useState<TableState>({
    simViewMode: "chart",
    tableSortKey: "time",
    tableSortAsc: true,
    tablePage: 0,
    tablePageSize: 100,
    visibleTableColumns: [],
  });

  const setParam = useCallback(<K extends keyof SimulationParams>(key: K, value: SimulationParams[K]) => {
    setParamsState((prev) => ({ ...prev, [key]: value }));
  }, []);

  const setTable = useCallback(<K extends keyof TableState>(key: K, value: TableState[K]) => {
    setTableState((prev) => ({ ...prev, [key]: value }));
  }, []);

  const buildOptions = useCallback((): JitValidateOptions => ({
    t_end: params.tEnd,
    dt: params.dt,
    solver: params.solver,
    output_interval: params.outputInterval,
    atol: params.atol,
    rtol: params.rtol,
  }), [params]);

  const validate = useCallback(async (code: string, modelName: string, projectDir: string | null) => {
    try {
      const result = await jitValidate({
        code,
        modelName,
        options: buildOptions(),
        projectDir: projectDir ?? undefined,
      });
      setJitResult(result);
      if (result.success) {
        log("JIT validation OK");
        setSelectedPlotVars((prev) =>
          prev.length ? prev : [...new Set([...(result.state_vars ?? []), ...(result.output_vars ?? [])])]
        );
      } else {
        log("JIT validation failed: " + result.errors.join("; "));
      }
      return result;
    } catch (e) {
      log("Error: " + String(e));
      setJitResult(null);
      return null;
    }
  }, [buildOptions, log]);

  const runSimulation = useCallback(async (code: string, modelName: string, projectDir: string | null) => {
    setSimLoading(true);
    setSimResult(null);
    log("Running simulation...");
    try {
      const result = await runSimulationApi({
        code,
        modelName,
        options: buildOptions(),
        projectDir: projectDir ?? undefined,
      });
      setSimResult(result);
      setSelectedPlotVars(Object.keys(result.series).filter((k) => k !== "time"));
      log("Simulation done. Points: " + (result.time?.length ?? 0));
    } catch (e) {
      log("Simulation error: " + String(e));
    } finally {
      setSimLoading(false);
    }
  }, [buildOptions, log]);

  const testAllMoFiles = useCallback(async (
    projectDir: string,
    moFiles: string[],
    pathToModelName: (p: string) => string
  ) => {
    setTestAllLoading(true);
    setTestAllResults(null);
    log(t("testAllRunning"));
    const opts = buildOptions();
    const results: { path: string; success: boolean; errors: string[] }[] = [];
    for (const path of moFiles) {
      try {
        const content = await readProjectFile(projectDir, path);
        const mn = pathToModelName(path);
        const result = await jitValidate({
          code: content,
          modelName: mn,
          options: opts,
          projectDir,
        });
        results.push({ path, success: result.success, errors: result.errors ?? [] });
      } catch (e) {
        results.push({ path, success: false, errors: [String(e)] });
      }
    }
    setTestAllResults(results);
    setTestAllLoading(false);
    const passed = results.filter((r) => r.success).length;
    const failed = results.filter((r) => !r.success).length;
    log(t("testAllSummary").replace("{passed}", String(passed)).replace("{failed}", String(failed)));
  }, [buildOptions, log]);

  const tableColumns = useMemo(() =>
    simResult ? ["time", ...Object.keys(simResult.series).filter((k) => k !== "time")] : [],
    [simResult]
  );

  const tableRows = useMemo(() =>
    simResult
      ? simResult.time.map((_, i) => {
          const row: Record<string, number> = { time: simResult.time[i] };
          for (const k of Object.keys(simResult.series)) {
            row[k] = simResult.series[k][i];
          }
          return row;
        })
      : [],
    [simResult]
  );

  const sortedTableRows = useMemo(() =>
    [...tableRows].sort((a, b) => {
      const va = a[tableState.tableSortKey];
      const vb = b[tableState.tableSortKey];
      if (va == null || vb == null) return 0;
      const cmp = va < vb ? -1 : va > vb ? 1 : 0;
      return tableState.tableSortAsc ? cmp : -cmp;
    }),
    [tableRows, tableState.tableSortKey, tableState.tableSortAsc]
  );

  useEffect(() => {
    if (tableColumns.length > 0) {
      setTableState((prev) => ({ ...prev, visibleTableColumns: [...tableColumns], tablePage: 0 }));
    } else {
      setTableState((prev) => ({ ...prev, visibleTableColumns: [] }));
    }
  }, [tableColumns.join(",")]);

  const plotTraces = useMemo(() =>
    simResult
      ? selectedPlotVars
          .filter((name) => simResult.series[name] != null)
          .map((name) => ({
            x: simResult.time,
            y: simResult.series[name],
            type: "scatter" as const,
            mode: "lines" as const,
            name,
          }))
      : [],
    [simResult, selectedPlotVars]
  );

  const allPlotVarNames = useMemo(() =>
    simResult != null
      ? Object.keys(simResult.series).filter((k) => k !== "time")
      : jitResult != null
        ? [...new Set([...(jitResult.state_vars ?? []), ...(jitResult.output_vars ?? [])])]
        : [],
    [simResult, jitResult]
  );

  const exportCSV = useCallback(() => {
    if (!simResult || tableColumns.length === 0) return;
    const header = tableColumns.join(",");
    const body = sortedTableRows.map((r) => tableColumns.map((c) => r[c]).join(",")).join("\n");
    const blob = new Blob([header + "\n" + body], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "simulation_result.csv";
    a.click();
    URL.revokeObjectURL(url);
  }, [simResult, tableColumns, sortedTableRows]);

  const exportJSON = useCallback(() => {
    if (!simResult || tableRows.length === 0) return;
    const blob = new Blob([JSON.stringify(tableRows, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "simulation_result.json";
    a.click();
    URL.revokeObjectURL(url);
  }, [simResult, tableRows]);

  return {
    params, setParam,
    jitResult, setJitResult,
    simResult,
    selectedPlotVars, setSelectedPlotVars,
    simLoading,
    testAllLoading, testAllResults,
    tableState, setTable,
    tableColumns, sortedTableRows,
    plotTraces, allPlotVarNames,
    validate, runSimulation, testAllMoFiles,
    exportCSV, exportJSON,
  };
}
