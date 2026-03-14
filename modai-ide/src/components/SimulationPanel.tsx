import { useEffect, useState } from "react";
import Plot from "react-plotly.js";
import { t } from "../i18n";
import { AppIcon } from "./Icon";
import { IconButton } from "./IconButton";
import { EquationGraphView } from "./EquationGraphView";
import type { JitValidateResult, SimulationResult } from "../types";

export interface TestAllResultItem {
  path: string;
  success: boolean;
  errors: string[];
}

export interface SimParams {
  tEnd: number;
  dt: number;
  solver: string;
  outputInterval: number;
  atol: number;
  rtol: number;
}

export interface SimTableState {
  simViewMode: "chart" | "table";
  tableSortKey: string;
  tableSortAsc: boolean;
  tablePage: number;
  tablePageSize: number;
  visibleTableColumns: string[];
}

export interface SimActions {
  onValidate: () => void;
  onRunSimulation: () => void;
  onTestAll: () => void;
  onExportCSV: () => void;
  onExportJSON: () => void;
  onSuggestFixWithAi: (msg: string) => void;
}

export interface SimResultData {
  jitResult: JitValidateResult | null;
  simResult: SimulationResult | null;
  simLoading: boolean;
  testAllLoading: boolean;
  testAllResults: TestAllResultItem[] | null;
  moFilesCount: number;
  logLines: string[];
  plotTraces: PlotTrace[];
  allPlotVarNames: string[];
  selectedPlotVars: string[];
  tableColumns: string[];
  sortedTableRows: Record<string, number>[];
}

type PlotTrace = { x: number[]; y: number[]; type: "scatter"; mode: "lines"; name: string };

type BottomTab = "verify" | "run" | "log" | "deps" | "vars";

const inputClass = "w-14 bg-[var(--surface)] border border-border px-1 text-sm rounded text-[var(--text)]";

function readThemeColor(name: string, fallback: string): string {
  if (typeof window === "undefined") return fallback;
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

function pathToModelName(relativePath: string | null | undefined): string {
  if (!relativePath) return "";
  const withoutExt = relativePath.replace(/\.mo$/i, "");
  return withoutExt.replace(/[/\\]/g, ".");
}

interface SimulationPanelProps {
  params: SimParams;
  onParamChange: <K extends keyof SimParams>(key: K, value: SimParams[K]) => void;
  tableState: SimTableState;
  onTableChange: <K extends keyof SimTableState>(key: K, value: SimTableState[K]) => void;
  actions: SimActions;
  data: SimResultData;
  setSelectedPlotVars: (v: string[] | ((prev: string[]) => string[])) => void;
  theme?: "dark" | "light";
  code?: string;
  openFilePath?: string | null;
  projectDir?: string | null;
  requestedTab?: BottomTab | null;
  onRequestedTabHandled?: () => void;
  onFocusSymbol?: (symbol: string) => void;
  selectedSymbol?: string | null;
}

export function SimulationPanel({
  params,
  onParamChange,
  tableState,
  onTableChange,
  actions,
  data,
  setSelectedPlotVars,
  theme = "dark",
  code = "",
  openFilePath = null,
  projectDir = null,
  requestedTab = null,
  onRequestedTabHandled,
  onFocusSymbol,
  selectedSymbol = null,
}: SimulationPanelProps) {
  const modelName = pathToModelName(openFilePath);
  const canShowDeps = Boolean(code && modelName && openFilePath?.toLowerCase().endsWith(".mo"));
  const [showSettings, setShowSettings] = useState(false);
  const [bottomTab, setBottomTab] = useState<BottomTab>("verify");

  const plotPaperBg = readThemeColor("--surface", theme === "light" ? "#f3f4f6" : "#1e1e1e");
  const plotBg = readThemeColor("--surface-elevated", theme === "light" ? "#ffffff" : "#2b2b2b");
  const plotFontColor = readThemeColor("--text", theme === "light" ? "#1f2937" : "#d4d4d4");

  const togglePlotVar = (name: string) => {
    setSelectedPlotVars((prev) => (prev.includes(name) ? prev.filter((v) => v !== name) : [...prev, name]));
  };
  const selectAllPlotVars = () => setSelectedPlotVars([...data.allPlotVarNames]);
  const clearPlotVars = () => setSelectedPlotVars([]);

  const totalTablePages = data.sortedTableRows.length > 0 ? Math.ceil(data.sortedTableRows.length / tableState.tablePageSize) : 0;
  const paginatedRows = data.sortedTableRows.slice(tableState.tablePage * tableState.tablePageSize, (tableState.tablePage + 1) * tableState.tablePageSize);
  const toggleTableColumn = (col: string) => {
    const prev = tableState.visibleTableColumns;
    const next = prev.includes(col)
      ? prev.filter((c) => c !== col)
      : [...prev, col].sort((a, b) => data.tableColumns.indexOf(a) - data.tableColumns.indexOf(b));
    onTableChange("visibleTableColumns", next);
  };
  const [showColumnsDropdown, setShowColumnsDropdown] = useState(false);
  const [logSearch, setLogSearch] = useState("");

  useEffect(() => {
    if (!requestedTab) return;
    setBottomTab(requestedTab);
    onRequestedTabHandled?.();
  }, [requestedTab, onRequestedTabHandled]);

  return (
    <div className="h-full border-t border-border flex flex-col shrink-0 overflow-hidden bg-surface-alt">
      <div className="border-b border-border flex flex-col">
        <div className="flex items-center gap-2 px-2 py-1 flex-wrap">
          <IconButton
            icon={<AppIcon name="validate" aria-hidden="true" />}
            onClick={actions.onValidate}
            title={t("jitValidate")}
            aria-label={t("jitValidate")}
          />
          <IconButton
            icon={<AppIcon name="gitCommit" aria-hidden="true" />}
            onClick={actions.onTestAll}
            disabled={data.testAllLoading || data.moFilesCount === 0}
            title={t("testAllMoFiles")}
            aria-label={t("testAllMoFiles")}
          />
          <IconButton
            icon={<AppIcon name="run" aria-hidden="true" />}
            variant="primary"
            onClick={actions.onRunSimulation}
            disabled={data.simLoading}
            title={t("run")}
            aria-label={t("run")}
          />
          <IconButton
            icon={<AppIcon name="simSettings" aria-hidden="true" />}
            active={showSettings}
            onClick={() => setShowSettings((s) => !s)}
            title={t("simSettings")}
            aria-label={t("simSettings")}
          />
        </div>
        {showSettings && (
          <div className="px-2 py-2 border-t border-border flex flex-wrap gap-x-4 gap-y-2 text-xs">
            <fieldset className="flex flex-wrap items-center gap-2 border border-border rounded px-2 py-1">
              <legend className="text-[var(--text-muted)]">{t("simGroupSimulation")}</legend>
              <label className="flex items-center gap-1"><span>{t("paramTEnd")}</span><input type="number" value={params.tEnd} onChange={(e) => onParamChange("tEnd", Number(e.target.value))} className={inputClass} /></label>
              <label className="flex items-center gap-1"><span>{t("paramDt")}</span><input type="number" step={0.001} value={params.dt} onChange={(e) => onParamChange("dt", Number(e.target.value))} className={inputClass} /></label>
              <label className="flex items-center gap-1"><span>{t("paramSolver")}</span><select value={params.solver} onChange={(e) => onParamChange("solver", e.target.value)} className={inputClass}><option value="rk4">rk4</option><option value="rk45">rk45</option><option value="implicit">{t("implicitSolver")}</option></select></label>
              <label className="flex items-center gap-1"><span>{t("paramOutputInterval")}</span><input type="number" step={0.001} value={params.outputInterval} onChange={(e) => onParamChange("outputInterval", Number(e.target.value))} className={inputClass} /></label>
            </fieldset>
            <fieldset className="flex flex-wrap items-center gap-2 border border-border rounded px-2 py-1">
              <legend className="text-[var(--text-muted)]">{t("simGroupTolerance")}</legend>
              <label className="flex items-center gap-1"><span>{t("paramAtol")}</span><input type="number" step={1e-12} value={params.atol} onChange={(e) => onParamChange("atol", Number(e.target.value))} className={inputClass} /></label>
              <label className="flex items-center gap-1"><span>{t("paramRtol")}</span><input type="number" step={1e-6} value={params.rtol} onChange={(e) => onParamChange("rtol", Number(e.target.value))} className={inputClass} /></label>
            </fieldset>
          </div>
        )}
      </div>
      <div className="flex items-center gap-1 px-2 py-0.5 border-b border-border shrink-0">
        <IconButton
          icon={<AppIcon name="validate" aria-hidden="true" />}
          variant="tab"
          size="xs"
          active={bottomTab === "verify"}
          onClick={() => setBottomTab("verify")}
          title={t("tabVerifyTest")}
          aria-label={t("tabVerifyTest")}
        />
        <IconButton
          icon={<AppIcon name="chart" aria-hidden="true" />}
          variant="tab"
          size="xs"
          active={bottomTab === "run"}
          onClick={() => setBottomTab("run")}
          title={t("tabRunResult")}
          aria-label={t("tabRunResult")}
        />
        <IconButton
          icon={<AppIcon name="table" aria-hidden="true" />}
          variant="tab"
          size="xs"
          active={bottomTab === "log"}
          onClick={() => setBottomTab("log")}
          title={t("tabLog")}
          aria-label={t("tabLog")}
        />
        <IconButton
          icon={<AppIcon name="variables" aria-hidden="true" />}
          variant="tab"
          size="xs"
          active={bottomTab === "vars"}
          onClick={() => setBottomTab("vars")}
          title={t("variablesBrowser")}
          aria-label={t("variablesBrowser")}
        />
        {canShowDeps && (
          <IconButton
            icon={<AppIcon name="link" aria-hidden="true" />}
            variant="tab"
            size="xs"
            active={bottomTab === "deps"}
            onClick={() => setBottomTab("deps")}
            title={t("tabDependencies")}
            aria-label={t("tabDependencies")}
          />
        )}
      </div>
      <div className="flex-1 min-h-0 flex overflow-hidden">
        {bottomTab === "verify" && (
        <div className="flex-1 overflow-auto p-2 text-xs font-mono scroll-vscode">
          {data.testAllResults != null && (() => {
            const lines: string[] = [];
            let passed = 0;
            let failed = 0;
            for (const r of data.testAllResults) {
              if (r.success) {
                lines.push("PASS " + r.path);
                passed += 1;
              } else {
                lines.push("FAIL " + r.path);
                failed += 1;
                for (const err of r.errors) {
                  lines.push("  " + err);
                }
              }
            }
            lines.push("---");
            lines.push(`Summary: ${passed} passed, ${failed} failed`);
            const regressionText = lines.join("\n");
            return (
              <div className="mb-1">
                <pre className="text-xs font-mono whitespace-pre-wrap break-words bg-black/20 p-1 rounded mb-1 max-h-48 overflow-auto scroll-vscode">{regressionText}</pre>
                <button
                  type="button"
                  className="px-2 py-0.5 text-xs rounded border theme-button-secondary"
                  onClick={() => void navigator.clipboard.writeText(regressionText)}
                >
                  {t("copyTestAllOutput")}
                </button>
                <div className="mt-1">
                  {data.testAllResults.map((r, i) => (
                    <div key={i} className={r.success ? "text-[var(--success-text)]" : "text-[var(--danger-text)]"}>
                      {r.success ? "\u2713" : "\u2717"} {r.path}
                      {!r.success && r.errors.length > 0 && <div className="pl-3 text-[var(--warning-text)]">{r.errors[0]}</div>}
                    </div>
                  ))}
                </div>
              </div>
            );
          })()}
          {data.jitResult && !data.jitResult.success && (
            <div className="text-[var(--danger-text)]">
              {data.jitResult.errors.map((e, i) => (
                <div key={i}>{e}</div>
              ))}
              <button
                type="button"
                onClick={() => actions.onSuggestFixWithAi("Fix the following Modelica compile error and suggest corrected code: " + data.jitResult!.errors.join(" "))}
                className="mt-1 px-2 py-0.5 bg-primary/80 hover:bg-primary text-white text-xs rounded"
              >
                {t("suggestFixWithAi")}
              </button>
            </div>
          )}
          {data.jitResult?.warnings?.map((w, i) => (
            <div key={i} className="text-[var(--warning-text)]">
              {w.path}:{w.line}:{w.column} {w.message}
            </div>
          ))}
          {data.logLines.slice(-20).map((line, i) => (
            <div key={i} className="text-[var(--text-muted)]">{line}</div>
          ))}
        </div>
        )}
        {bottomTab === "run" && (
        <>
          <div className="w-40 shrink-0 border-r border-border overflow-auto p-1 text-xs flex flex-col">
            <div className="text-[var(--text-muted)] font-medium mb-1">{t("variablesSelect")}</div>
            {data.allPlotVarNames.length > 0 ? (
              <>
                <div className="flex gap-1 mb-1">
                  <IconButton
                    icon={<AppIcon name="stage" aria-hidden="true" />}
                    size="xs"
                    onClick={selectAllPlotVars}
                    title={t("selectAllVariables")}
                    aria-label={t("selectAllVariables")}
                  />
                  <IconButton
                    icon={<AppIcon name="unstage" aria-hidden="true" />}
                    size="xs"
                    onClick={clearPlotVars}
                    title={t("clearVariableSelection")}
                    aria-label={t("clearVariableSelection")}
                  />
                </div>
                <div className="space-y-0.5">
                  {data.allPlotVarNames.map((name) => (
                    <label key={name} className="flex items-center gap-1 cursor-pointer truncate" title={name}>
                      <input type="checkbox" checked={data.selectedPlotVars.includes(name)} onChange={() => togglePlotVar(name)} className="shrink-0" />
                      <span className="truncate">{name}</span>
                    </label>
                  ))}
                </div>
              </>
            ) : (
              <div className="text-[var(--text-muted)] text-xs">{t("runJitFirst")}</div>
            )}
          </div>
          <div className="flex-1 min-w-0 flex flex-col min-h-0">
            <div className="flex items-center gap-2 px-1 py-0.5 border-b border-border shrink-0 flex-wrap bg-surface-alt z-10">
              <IconButton
                icon={<AppIcon name="chart" aria-hidden="true" />}
                size="xs"
                active={tableState.simViewMode === "chart"}
                onClick={() => onTableChange("simViewMode", "chart")}
                title={t("chartView")}
                aria-label={t("chartView")}
              />
              <IconButton
                icon={<AppIcon name="table" aria-hidden="true" />}
                size="xs"
                active={tableState.simViewMode === "table"}
                onClick={() => onTableChange("simViewMode", "table")}
                title={t("tableView")}
                aria-label={t("tableView")}
              />
              {data.simResult && (
                <>
                  <button type="button" className="px-2 py-0.5 text-xs rounded border theme-button-secondary text-[var(--text-muted)]" onClick={actions.onExportCSV}>{t("exportCSV")}</button>
                  <button type="button" className="px-2 py-0.5 text-xs rounded border theme-button-secondary text-[var(--text-muted)]" onClick={actions.onExportJSON}>{t("exportJSON")}</button>
                </>
              )}
              {tableState.simViewMode === "table" && data.simResult && (
                <>
                  <span className="text-[var(--text-muted)] text-xs">{t("tablePageSize")}</span>
                  <select value={tableState.tablePageSize} onChange={(e) => { onTableChange("tablePageSize", Number(e.target.value)); onTableChange("tablePage", 0); }} className="w-14 text-xs rounded bg-surface border border-border px-1">
                    <option value={50}>50</option>
                    <option value={100}>100</option>
                    <option value={200}>200</option>
                    <option value={500}>500</option>
                  </select>
                  <IconButton
                    icon={<AppIcon name="prev" aria-hidden="true" />}
                    size="xs"
                    className="border theme-button-secondary text-[var(--text-muted)] disabled:opacity-50"
                    disabled={tableState.tablePage <= 0}
                    onClick={() => onTableChange("tablePage", Math.max(0, tableState.tablePage - 1))}
                    title={t("previousPage")}
                    aria-label={t("previousPage")}
                  />
                  <span className="text-xs text-[var(--text-muted)]">{(tableState.tablePage + 1) + " / " + (totalTablePages || 1)}</span>
                  <IconButton
                    icon={<AppIcon name="next" aria-hidden="true" />}
                    size="xs"
                    className="border theme-button-secondary text-[var(--text-muted)] disabled:opacity-50"
                    disabled={tableState.tablePage >= totalTablePages - 1}
                    onClick={() => onTableChange("tablePage", Math.min(totalTablePages - 1, tableState.tablePage + 1))}
                    title={t("nextPage")}
                    aria-label={t("nextPage")}
                  />
                  <div className="relative">
                    <IconButton
                      icon={<AppIcon name="columns" aria-hidden="true" />}
                      size="xs"
                      className="border theme-button-secondary text-[var(--text-muted)]"
                      onClick={() => setShowColumnsDropdown((s) => !s)}
                      title={t("columnsSelect")}
                      aria-label={t("columnsSelect")}
                    />
                    {showColumnsDropdown && (
                      <div className="absolute left-0 top-full mt-0.5 z-10 bg-surface-alt border border-border rounded shadow-lg p-1 max-h-48 overflow-auto">
                        {data.tableColumns.map((col) => (
                          <label key={col} className="flex items-center gap-1 cursor-pointer text-xs block whitespace-nowrap">
                            <input type="checkbox" checked={tableState.visibleTableColumns.includes(col)} onChange={() => toggleTableColumn(col)} />
                            {col}
                          </label>
                        ))}
                        <button type="button" className="mt-1 w-full text-xs rounded border theme-button-secondary" onClick={() => setShowColumnsDropdown(false)}>{t("closeTab")}</button>
                      </div>
                    )}
                  </div>
                </>
              )}
            </div>
            <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
              {tableState.simViewMode === "chart" ? (
                data.plotTraces.length > 0 ? (
                  <div className="min-h-full flex flex-col">
                    <Plot
                      data={data.plotTraces}
                      layout={{
                        margin: { t: 40, r: 8, b: 24, l: 40 },
                      paper_bgcolor: plotPaperBg,
                      plot_bgcolor: plotBg,
                      font: { color: plotFontColor, size: 10 },
                      xaxis: { title: "time" },
                      yaxis: { title: "" },
                      showlegend: true,
                      legend: { x: 1, y: 1, xanchor: "right" },
                      dragmode: "zoom",
                    }}
                    config={{ responsive: true, scrollZoom: true, displayModeBar: true, modeBarButtonsToRemove: [] }}
                    style={{ width: "100%", minHeight: "200px" }}
                    useResizeHandler
                  />
                  </div>
                ) : (
                  <div className="flex items-center justify-center h-full text-[var(--text-muted)] text-sm">{t("runSimulationToSeePlot")}</div>
                )
              ) : data.simResult ? (
                <div className="min-h-0 flex flex-col flex-1 overflow-hidden">
                  <div className="overflow-auto flex-1 min-h-0 scroll-vscode relative">
                    <table className="w-full text-xs border-collapse">
                      <thead className="sticky top-0 z-20 bg-surface-alt shadow-[0_1px_0_0_var(--border)]">
                        <tr>
                          {(tableState.visibleTableColumns.length ? tableState.visibleTableColumns : data.tableColumns).map((col) => (
                            <th
                              key={col}
                              className="border border-border px-2 py-1 text-left cursor-pointer hover:bg-[var(--surface-hover)] bg-surface-alt"
                            onClick={() => {
                              onTableChange("tableSortKey", col);
                              onTableChange("tableSortAsc", tableState.tableSortKey === col ? !tableState.tableSortAsc : true);
                            }}
                          >
                            {col} {tableState.tableSortKey === col ? (tableState.tableSortAsc ? "\u2191" : "\u2193") : ""}
                          </th>
                        ))}
                        </tr>
                      </thead>
                      <tbody>
                        {paginatedRows.map((row, i) => (
                          <tr key={tableState.tablePage * tableState.tablePageSize + i}>
                            {(tableState.visibleTableColumns.length ? tableState.visibleTableColumns : data.tableColumns).map((col) => (
                              <td key={col} className="border border-border px-2 py-0.5 font-mono">
                                {typeof row[col] === "number" ? (row[col] as number).toExponential(4) : row[col]}
                              </td>
                            ))}
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </div>
              ) : (
                <div className="flex items-center justify-center h-full text-[var(--text-muted)] text-sm">{t("runSimulationToSeePlot")}</div>
              )}
            </div>
          </div>
        </>
        )}
        {bottomTab === "log" && (
        <div className="flex-1 flex flex-col min-h-0">
          <div className="shrink-0 flex items-center gap-2 px-2 py-1 border-b border-border">
            <input type="text" placeholder={t("tableSearch")} value={logSearch} onChange={(e) => setLogSearch(e.target.value)} className="flex-1 max-w-xs text-xs rounded bg-surface border border-border px-2 py-0.5" />
          </div>
          <div className="flex-1 overflow-auto p-2 text-xs font-mono scroll-vscode">
            {data.logLines.length === 0 ? <div className="text-[var(--text-muted)]">{t("tabLog")}</div> : data.logLines.filter((line) => !logSearch.trim() || line.includes(logSearch.trim())).map((line, i) => (
              <div key={i} className="text-gray-500">{line}</div>
            ))}
          </div>
        </div>
        )}
        {bottomTab === "vars" && (
        <div className="flex-1 overflow-auto p-3 text-xs space-y-3">
          <div>
            <div className="text-[var(--text-muted)] mb-1">state</div>
            <div className="space-y-0.5">
              {(data.jitResult?.state_vars ?? []).map((name) => (
                <button
                  key={`state:${name}`}
                  type="button"
                  className={`block w-full rounded px-2 py-1 text-left font-mono ${
                    selectedSymbol === name ? "bg-primary/20 text-primary" : "hover:bg-white/5"
                  }`}
                  onClick={() => onFocusSymbol?.(name)}
                >
                  {name}
                </button>
              ))}
            </div>
          </div>
          <div>
            <div className="text-[var(--text-muted)] mb-1">output</div>
            <div className="space-y-0.5">
              {(data.jitResult?.output_vars ?? []).map((name) => (
                <button
                  key={`output:${name}`}
                  type="button"
                  className={`block w-full rounded px-2 py-1 text-left font-mono ${
                    selectedSymbol === name ? "bg-primary/20 text-primary" : "hover:bg-white/5"
                  }`}
                  onClick={() => onFocusSymbol?.(name)}
                >
                  {name}
                </button>
              ))}
            </div>
          </div>
        </div>
        )}
        {bottomTab === "deps" && canShowDeps && (
        <div className="flex-1 min-h-0 flex flex-col">
          <EquationGraphView code={code} modelName={modelName} projectDir={projectDir} />
        </div>
        )}
      </div>
    </div>
  );
}
