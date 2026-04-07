import { useState, useCallback, useEffect, useMemo } from "react";
import { listen } from "@tauri-apps/api/event";
import type { JitValidateOptions, JitValidateResult, SimulationResult } from "../types";
import {
  formatMonitorReplayLine,
  getMonitorEvents,
  jitValidateV2,
  readProjectFile,
  runSimulationV2,
} from "../api/tauri";
import { t, tf } from "../i18n";
import {
  buildSimulationChartMeta,
  buildSimulationChartSeries,
  buildSimulationTableColumns,
  buildSimulationTableRows,
  listSimulationPlotVarNames,
  sortSimulationTableRows,
} from "./simulationSelectors";

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

export function useSimulation(
  log: (msg: string) => void,
  validationDefaultTier?: string | null,
  eqExpandParallelMode?: "off" | "guarded" | "on" | null
) {
  useEffect(() => {
    let disposed = false;
    void getMonitorEvents(undefined, 40)
      .then((rows) => {
        if (disposed || rows.length === 0) return;
        log("[monitor-replay] restored recent runtime events:");
        for (const r of rows) {
          log(formatMonitorReplayLine(r));
        }
      })
      .catch(() => {});
    return () => {
      disposed = true;
    };
  }, [log]);

  useEffect(() => {
    let isDisposed = false;
    let unlisten: (() => void) | null = null;
    void listen<{
      category?: "control" | "progress" | "error";
      task?: string;
      stage?: string;
      elapsedSec?: number;
      message?: string;
      currentStep?: number;
      totalSteps?: number;
      reason?: string;
    }>("modai-jit-progress", (event) => {
      if (isDisposed) return;
      const payload = event.payload ?? {};
      const category = payload.category ?? "progress";
      const task = payload.task ?? "task";
      const stage = payload.stage ?? "running";
      const elapsed = payload.elapsedSec != null ? ` (${payload.elapsedSec}s)` : "";
      const reason = payload.reason ? ` reason=${payload.reason}` : "";
      const stepPart =
        payload.currentStep != null && payload.totalSteps != null
          ? ` step=${payload.currentStep}/${payload.totalSteps}`
          : "";
      const msg = payload.message?.trim() || `${task} ${stage}${elapsed}${stepPart}${reason}`;
      log(`[${category}] [backend:${task}] ${msg}`);
    }).then((off) => {
      if (isDisposed) off();
      else unlisten = off;
    }).catch(() => {});
    return () => {
      isDisposed = true;
      unlisten?.();
    };
  }, [log]);

  const [params, setParamsState] = useState<SimulationParams>({
    tEnd: 2, dt: 0.01, solver: "rk45", outputInterval: 0.05, atol: 1e-6, rtol: 1e-3,
  });
  const [jitResult, setJitResult] = useState<JitValidateResult | null>(null);
  const [simResult, setSimResult] = useState<SimulationResult | null>(null);
  const [selectedPlotVars, setSelectedPlotVars] = useState<string[]>([]);
  const [simLoading, setSimLoading] = useState(false);
  const [validateLoading, setValidateLoading] = useState(false);

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

  const tierTrim = validationDefaultTier?.trim();
  const eqModeRaw = (eqExpandParallelMode ?? "off").trim().toLowerCase();
  const eqMode: "off" | "guarded" | "on" =
    eqModeRaw === "guarded" || eqModeRaw === "on" ? eqModeRaw : "off";
  const buildOptions = useCallback((): JitValidateOptions => ({
    t_end: params.tEnd,
    dt: params.dt,
    solver: params.solver,
    output_interval: params.outputInterval,
    atol: params.atol,
    rtol: params.rtol,
    ...(tierTrim ? { validationTier: tierTrim } : {}),
    eqExpandParallelMode: eqMode,
  }), [eqMode, params, tierTrim]);

  const beginHeartbeat = useCallback((label: string, intervalMs = 5000) => {
    const startedAt = Date.now();
    log(`${label} started...`);
    const timer = setInterval(() => {
      const elapsed = Math.max(1, Math.floor((Date.now() - startedAt) / 1000));
      log(`${label} running (${elapsed}s)...`);
    }, intervalMs);
    return () => {
      clearInterval(timer);
      return Math.max(1, Math.floor((Date.now() - startedAt) / 1000));
    };
  }, [log]);

  const validate = useCallback(async (code: string, modelName: string, projectDir: string | null) => {
    setValidateLoading(true);
    log(tf("jitValidateLogStart", { model: modelName }));
    log(`eqExpandParallelMode(validate): ${eqMode}`);
    const t0 = typeof performance !== "undefined" ? performance.now() : 0;
    const finishHeartbeat = beginHeartbeat("Validation");
    try {
      const result = await jitValidateV2({
        code,
        modelName,
        options: buildOptions(),
        projectDir: projectDir ?? undefined,
      });
      const resolved = result.data ?? {
        schema_version: result.meta.schemaVersion,
        success: false,
        warnings: [],
        errors: result.errors.map((e) => e.message),
        diagnostics: result.errors.map((e) => ({
          code: e.code,
          message: e.message,
          path: e.path ?? undefined,
          line: e.line ?? undefined,
          column: e.column ?? undefined,
        })),
        state_vars: [],
        output_vars: [],
        compile_trace: [],
        validation_stop_phase: null,
        validation_partial: false,
      };
      for (const line of resolved.compile_trace ?? []) {
        log(line);
      }
      if (resolved.warnings.length > 0) {
        log(tf("jitValidateLogWarningsHeader", { count: resolved.warnings.length }));
        for (const w of resolved.warnings) {
          log(`${w.path}:${w.line}:${w.column} ${w.message}`);
        }
      }
      setJitResult(resolved);
      const elapsedMs =
        typeof performance !== "undefined" ? Math.round(performance.now() - t0) : 0;
      if (resolved.success) {
        log(tf("jitValidateLogDoneOk", { ms: elapsedMs }));
        if (resolved.validation_partial && resolved.validation_stop_phase) {
          log(`Validation tier: ${resolved.validation_stop_phase} (partial, no JIT).`);
        }
        setSelectedPlotVars((prev) =>
          prev.length ? prev : [...new Set([...(resolved.state_vars ?? []), ...(resolved.output_vars ?? [])])]
        );
      } else {
        log(tf("jitValidateLogDoneFail", { ms: elapsedMs }));
        for (const errLine of resolved.errors) {
          log(errLine);
        }
      }
      const elapsedSec = finishHeartbeat();
      log(`Validation completed in ${elapsedSec}s.`);
      return resolved;
    } catch (e) {
      const elapsedSec = finishHeartbeat();
      log(tf("jitValidateLogException", { message: String(e) }));
      log(`Validation failed after ${elapsedSec}s.`);
      setJitResult(null);
      return null;
    } finally {
      setValidateLoading(false);
    }
  }, [beginHeartbeat, buildOptions, eqMode, log]);

  const runSimulation = useCallback(async (code: string, modelName: string, projectDir: string | null) => {
    setSimLoading(true);
    setSimResult(null);
    log(`eqExpandParallelMode(simulate): ${eqMode}`);
    const finishHeartbeat = beginHeartbeat("Simulation");
    try {
      const result = await runSimulationV2({
        code,
        modelName,
        options: buildOptions(),
        projectDir: projectDir ?? undefined,
      });
      if (!result.ok || !result.data) {
        const errorText = result.errors.map((e) => `${e.code}: ${e.message}`).join("; ");
        throw new Error(errorText || "Simulation failed");
      }
      const sim = result.data;
      setSimResult(sim);
      setSelectedPlotVars(Object.keys(sim.series).filter((k) => k !== "time"));
      log("Simulation done. Points: " + (sim.time?.length ?? 0));
      const elapsedSec = finishHeartbeat();
      log(`Simulation completed in ${elapsedSec}s.`);
    } catch (e) {
      const elapsedSec = finishHeartbeat();
      log("Simulation error: " + String(e));
      log(`Simulation failed after ${elapsedSec}s.`);
    } finally {
      setSimLoading(false);
    }
  }, [beginHeartbeat, buildOptions, eqMode, log]);

  const testAllMoFiles = useCallback(async (
    projectDir: string,
    moFiles: string[],
    pathToModelName: (p: string) => string
  ) => {
    setTestAllLoading(true);
    setTestAllResults(null);
    log(t("testAllRunning"));
    const finishHeartbeat = beginHeartbeat("Test-all");
    const opts = buildOptions();
    const results: { path: string; success: boolean; errors: string[] }[] = [];
    let processed = 0;
    for (const path of moFiles) {
      try {
        const content = await readProjectFile(projectDir, path);
        const mn = pathToModelName(path);
        const envelope = await jitValidateV2({
          code: content,
          modelName: mn,
          options: opts,
          projectDir,
        });
        const result = envelope.data;
        if (result) {
          results.push({ path, success: result.success, errors: result.errors ?? [] });
        } else {
          results.push({
            path,
            success: false,
            errors: envelope.errors.map((e) => `${e.code}: ${e.message}`),
          });
        }
      } catch (e) {
        results.push({ path, success: false, errors: [String(e)] });
      }
      processed++;
      if (processed % 10 === 0 || processed === moFiles.length) {
        log(`Test-all progress: ${processed}/${moFiles.length}`);
      }
    }
    setTestAllResults(results);
    setTestAllLoading(false);
    const passed = results.filter((r) => r.success).length;
    const failed = results.filter((r) => !r.success).length;
    log(t("testAllSummary").replace("{passed}", String(passed)).replace("{failed}", String(failed)));
    const elapsedSec = finishHeartbeat();
    log(`Test-all completed in ${elapsedSec}s.`);
  }, [beginHeartbeat, buildOptions, log]);

  const tableColumns = useMemo(() => buildSimulationTableColumns(simResult), [simResult]);

  const tableRows = useMemo(() => buildSimulationTableRows(simResult), [simResult]);

  const sortedTableRows = useMemo(
    () => sortSimulationTableRows(tableRows, tableState.tableSortKey, tableState.tableSortAsc),
    [tableRows, tableState.tableSortKey, tableState.tableSortAsc]
  );

  useEffect(() => {
    if (tableColumns.length > 0) {
      setTableState((prev) => ({ ...prev, visibleTableColumns: [...tableColumns], tablePage: 0 }));
    } else {
      setTableState((prev) => ({ ...prev, visibleTableColumns: [] }));
    }
  }, [tableColumns.join(",")]);

  const plotSeries = useMemo(
    () => buildSimulationChartSeries(simResult, selectedPlotVars),
    [simResult, selectedPlotVars]
  );

  const chartMeta = useMemo(
    () => buildSimulationChartMeta(simResult, plotSeries),
    [simResult, plotSeries]
  );

  const allPlotVarNames = useMemo(
    () => listSimulationPlotVarNames(simResult, jitResult),
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
    validateLoading,
    simLoading,
    testAllLoading, testAllResults,
    tableState, setTable,
    tableColumns, sortedTableRows,
    plotSeries, chartMeta, allPlotVarNames,
    validate, runSimulation, testAllMoFiles,
    exportCSV, exportJSON,
  };
}

export type ModelicaSimulationApi = ReturnType<typeof useSimulation>;
