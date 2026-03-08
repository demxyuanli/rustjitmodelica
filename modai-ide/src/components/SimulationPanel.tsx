import { useState } from "react";
import Plot from "react-plotly.js";
import { t } from "../i18n";
import type { JitValidateResult, SimulationResult } from "../types";

export interface TestAllResultItem {
  path: string;
  success: boolean;
  errors: string[];
}

type BottomTab = "verify" | "run" | "log";

const inputClass = "w-14 bg-[var(--surface)] border border-border px-1 text-sm rounded text-[var(--text)]";

interface SimulationPanelProps {
  tEnd: number;
  setTEnd: (v: number) => void;
  dt: number;
  setDt: (v: number) => void;
  solver: string;
  setSolver: (v: string) => void;
  outputInterval: number;
  setOutputInterval: (v: number) => void;
  atol: number;
  setAtol: (v: number) => void;
  rtol: number;
  setRtol: (v: number) => void;
  onValidate: () => void;
  onTestAllMoFiles: () => void;
  testAllLoading: boolean;
  testAllResults: TestAllResultItem[] | null;
  moFilesCount: number;
  onRunSimulation: () => void;
  simLoading: boolean;
  jitResult: JitValidateResult | null;
  logLines: string[];
  simResult: SimulationResult | null;
  simViewMode: "chart" | "table";
  setSimViewMode: (v: "chart" | "table") => void;
  tableSortKey: string;
  setTableSortKey: (v: string) => void;
  tableSortAsc: boolean;
  setTableSortAsc: (v: boolean) => void;
  tableColumns: string[];
  sortedTableRows: Record<string, number>[];
  tablePage: number;
  setTablePage: (v: number) => void;
  tablePageSize: number;
  setTablePageSize: (v: number) => void;
  visibleTableColumns: string[];
  setVisibleTableColumns: (v: string[] | ((prev: string[]) => string[])) => void;
  onExportCSV: () => void;
  onExportJSON: () => void;
  plotTraces: { x: number[]; y: number[]; type: "scatter"; mode: "lines"; name: string }[];
  onSuggestFixWithAi: (msg: string) => void;
  selectedPlotVars: string[];
  setSelectedPlotVars: (v: string[] | ((prev: string[]) => string[])) => void;
  allPlotVarNames: string[];
  theme?: "dark" | "light";
}

export function SimulationPanel({
  tEnd,
  setTEnd,
  dt,
  setDt,
  solver,
  setSolver,
  outputInterval,
  setOutputInterval,
  atol,
  setAtol,
  rtol,
  setRtol,
  onValidate,
  onTestAllMoFiles,
  testAllLoading,
  testAllResults,
  moFilesCount,
  onRunSimulation,
  simLoading,
  jitResult,
  logLines,
  simResult,
  simViewMode,
  setSimViewMode,
  tableSortKey,
  setTableSortKey,
  tableSortAsc,
  setTableSortAsc,
  tableColumns,
  sortedTableRows,
  tablePage,
  setTablePage,
  tablePageSize,
  setTablePageSize,
  visibleTableColumns,
  setVisibleTableColumns,
  onExportCSV,
  onExportJSON,
  plotTraces,
  onSuggestFixWithAi,
  selectedPlotVars,
  setSelectedPlotVars,
  allPlotVarNames,
  theme = "dark",
}: SimulationPanelProps) {
  const [showSettings, setShowSettings] = useState(false);
  const [bottomTab, setBottomTab] = useState<BottomTab>("verify");

  const plotPaperBg = theme === "light" ? "#f3f4f6" : "#1e1e1e";
  const plotBg = theme === "light" ? "#e5e7eb" : "#252526";
  const plotFontColor = theme === "light" ? "#1f2937" : "#d4d4d4";

  const togglePlotVar = (name: string) => {
    setSelectedPlotVars((prev) => (prev.includes(name) ? prev.filter((v) => v !== name) : [...prev, name]));
  };
  const selectAllPlotVars = () => setSelectedPlotVars([...allPlotVarNames]);
  const clearPlotVars = () => setSelectedPlotVars([]);

  const totalTablePages = sortedTableRows.length > 0 ? Math.ceil(sortedTableRows.length / tablePageSize) : 0;
  const paginatedRows = sortedTableRows.slice(tablePage * tablePageSize, (tablePage + 1) * tablePageSize);
  const toggleTableColumn = (col: string) => {
    setVisibleTableColumns((prev) =>
      prev.includes(col) ? prev.filter((c) => c !== col) : [...prev, col].sort((a, b) => tableColumns.indexOf(a) - tableColumns.indexOf(b))
    );
  };
  const [showColumnsDropdown, setShowColumnsDropdown] = useState(false);
  const [logSearch, setLogSearch] = useState("");

  return (
    <div className="h-full border-t border-border flex flex-col shrink-0 overflow-hidden bg-surface-alt">
      <div className="border-b border-border flex flex-col">
        <div className="flex items-center gap-2 px-2 py-1 flex-wrap">
          <button type="button" onClick={onValidate} className="px-2 py-1 bg-gray-600 hover:bg-gray-500 text-sm rounded">{t("jitValidate")}</button>
          <button type="button" onClick={onTestAllMoFiles} disabled={testAllLoading || moFilesCount === 0} className="px-2 py-1 bg-gray-600 hover:bg-gray-500 text-sm rounded disabled:opacity-50" title={moFilesCount === 0 ? "" : undefined}>{testAllLoading ? t("testAllRunning") : t("testAllMoFiles")}</button>
          <button type="button" onClick={onRunSimulation} disabled={simLoading} className="px-2 py-1 bg-primary hover:bg-blue-600 text-sm disabled:opacity-50 rounded">{simLoading ? "..." : t("run")}</button>
          <button type="button" onClick={() => setShowSettings((s) => !s)} className={`px-2 py-1 text-sm rounded ${showSettings ? "bg-gray-600 text-white" : "bg-surface text-[var(--text-muted)] hover:bg-gray-600"}`}>{t("simSettings")}</button>
        </div>
        {showSettings && (
          <div className="px-2 py-2 border-t border-border flex flex-wrap gap-x-4 gap-y-2 text-xs">
            <fieldset className="flex flex-wrap items-center gap-2 border border-border rounded px-2 py-1">
              <legend className="text-[var(--text-muted)]">{t("simGroupSimulation")}</legend>
              <label className="flex items-center gap-1"><span>{t("paramTEnd")}</span><input type="number" value={tEnd} onChange={(e) => setTEnd(Number(e.target.value))} className={inputClass} /></label>
              <label className="flex items-center gap-1"><span>{t("paramDt")}</span><input type="number" step={0.001} value={dt} onChange={(e) => setDt(Number(e.target.value))} className={inputClass} /></label>
              <label className="flex items-center gap-1"><span>{t("paramSolver")}</span><select value={solver} onChange={(e) => setSolver(e.target.value)} className={inputClass}><option value="rk4">rk4</option><option value="rk45">rk45</option></select></label>
              <label className="flex items-center gap-1"><span>{t("paramOutputInterval")}</span><input type="number" step={0.001} value={outputInterval} onChange={(e) => setOutputInterval(Number(e.target.value))} className={inputClass} /></label>
            </fieldset>
            <fieldset className="flex flex-wrap items-center gap-2 border border-border rounded px-2 py-1">
              <legend className="text-[var(--text-muted)]">{t("simGroupTolerance")}</legend>
              <label className="flex items-center gap-1"><span>{t("paramAtol")}</span><input type="number" step={1e-12} value={atol} onChange={(e) => setAtol(Number(e.target.value))} className={inputClass} /></label>
              <label className="flex items-center gap-1"><span>{t("paramRtol")}</span><input type="number" step={1e-6} value={rtol} onChange={(e) => setRtol(Number(e.target.value))} className={inputClass} /></label>
            </fieldset>
          </div>
        )}
      </div>
      <div className="flex items-center gap-1 px-2 py-0.5 border-b border-border shrink-0">
        <button type="button" className={`px-2 py-0.5 text-xs rounded ${bottomTab === "verify" ? "bg-primary text-white" : "bg-surface-alt text-[var(--text-muted)] hover:bg-gray-600"}`} onClick={() => setBottomTab("verify")}>{t("tabVerifyTest")}</button>
        <button type="button" className={`px-2 py-0.5 text-xs rounded ${bottomTab === "run" ? "bg-primary text-white" : "bg-surface-alt text-[var(--text-muted)] hover:bg-gray-600"}`} onClick={() => setBottomTab("run")}>{t("tabRunResult")}</button>
        <button type="button" className={`px-2 py-0.5 text-xs rounded ${bottomTab === "log" ? "bg-primary text-white" : "bg-surface-alt text-[var(--text-muted)] hover:bg-gray-600"}`} onClick={() => setBottomTab("log")}>{t("tabLog")}</button>
      </div>
      <div className="flex-1 min-h-0 flex overflow-hidden">
        {bottomTab === "verify" && (
        <div className="flex-1 overflow-auto p-2 text-xs font-mono scroll-vscode">
          {testAllResults != null && (() => {
            const lines: string[] = [];
            let passed = 0;
            let failed = 0;
            for (const r of testAllResults) {
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
                  className="px-2 py-0.5 text-xs rounded bg-gray-600 hover:bg-gray-500"
                  onClick={() => void navigator.clipboard.writeText(regressionText)}
                >
                  {t("copyTestAllOutput")}
                </button>
                <div className="mt-1">
                  {testAllResults.map((r, i) => (
                    <div key={i} className={r.success ? "text-green-500" : "text-red-400"}>
                      {r.success ? "\u2713" : "\u2717"} {r.path}
                      {!r.success && r.errors.length > 0 && <div className="pl-3 text-amber-400">{r.errors[0]}</div>}
                    </div>
                  ))}
                </div>
              </div>
            );
          })()}
          {jitResult && !jitResult.success && (
            <div className="text-red-400">
              {jitResult.errors.map((e, i) => (
                <div key={i}>{e}</div>
              ))}
              <button
                type="button"
                onClick={() => onSuggestFixWithAi("Fix the following Modelica compile error and suggest corrected code: " + jitResult.errors.join(" "))}
                className="mt-1 px-2 py-0.5 bg-primary/80 hover:bg-primary text-white text-xs rounded"
              >
                {t("suggestFixWithAi")}
              </button>
            </div>
          )}
          {jitResult?.warnings?.map((w, i) => (
            <div key={i} className="text-amber-400">
              {w.path}:{w.line}:{w.column} {w.message}
            </div>
          ))}
          {logLines.slice(-20).map((line, i) => (
            <div key={i} className="text-gray-500">{line}</div>
          ))}
        </div>
        )}
        {bottomTab === "run" && (
        <>
          <div className="w-40 shrink-0 border-r border-border overflow-auto p-1 text-xs flex flex-col">
            <div className="text-[var(--text-muted)] font-medium mb-1">{t("variablesSelect")}</div>
            {allPlotVarNames.length > 0 ? (
              <>
                <div className="flex gap-1 mb-1">
                  <button type="button" className="px-1 py-0.5 rounded bg-surface hover:bg-gray-600 text-[10px]" onClick={selectAllPlotVars}>All</button>
                  <button type="button" className="px-1 py-0.5 rounded bg-surface hover:bg-gray-600 text-[10px]" onClick={clearPlotVars}>None</button>
                </div>
                <div className="space-y-0.5">
                  {allPlotVarNames.map((name) => (
                    <label key={name} className="flex items-center gap-1 cursor-pointer truncate" title={name}>
                      <input type="checkbox" checked={selectedPlotVars.includes(name)} onChange={() => togglePlotVar(name)} className="shrink-0" />
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
              <button type="button" className={`px-2 py-0.5 text-xs rounded ${simViewMode === "chart" ? "bg-primary text-white" : "bg-surface-alt text-[var(--text-muted)]"}`} onClick={() => setSimViewMode("chart")}>{t("chartView")}</button>
              <button type="button" className={`px-2 py-0.5 text-xs rounded ${simViewMode === "table" ? "bg-primary text-white" : "bg-surface-alt text-[var(--text-muted)]"}`} onClick={() => setSimViewMode("table")}>{t("tableView")}</button>
              {simResult && (
                <>
                  <button type="button" className="px-2 py-0.5 text-xs rounded bg-surface-alt text-[var(--text-muted)] hover:bg-gray-600" onClick={onExportCSV}>{t("exportCSV")}</button>
                  <button type="button" className="px-2 py-0.5 text-xs rounded bg-surface-alt text-[var(--text-muted)] hover:bg-gray-600" onClick={onExportJSON}>{t("exportJSON")}</button>
                </>
              )}
              {simViewMode === "table" && simResult && (
                <>
                  <span className="text-[var(--text-muted)] text-xs">{t("tablePageSize")}</span>
                  <select value={tablePageSize} onChange={(e) => { setTablePageSize(Number(e.target.value)); setTablePage(0); }} className="w-14 text-xs rounded bg-surface border border-border px-1">
                    <option value={50}>50</option>
                    <option value={100}>100</option>
                    <option value={200}>200</option>
                    <option value={500}>500</option>
                  </select>
                  <button type="button" className="px-2 py-0.5 text-xs rounded bg-surface-alt text-[var(--text-muted)] hover:bg-gray-600 disabled:opacity-50" disabled={tablePage <= 0} onClick={() => setTablePage(Math.max(0, tablePage - 1))}>Prev</button>
                  <span className="text-xs text-[var(--text-muted)]">{(tablePage + 1) + " / " + (totalTablePages || 1)}</span>
                  <button type="button" className="px-2 py-0.5 text-xs rounded bg-surface-alt text-[var(--text-muted)] hover:bg-gray-600 disabled:opacity-50" disabled={tablePage >= totalTablePages - 1} onClick={() => setTablePage(Math.min(totalTablePages - 1, tablePage + 1))}>Next</button>
                  <div className="relative">
                    <button type="button" className="px-2 py-0.5 text-xs rounded bg-surface-alt text-[var(--text-muted)] hover:bg-gray-600" onClick={() => setShowColumnsDropdown((s) => !s)}>{t("columnsSelect")}</button>
                    {showColumnsDropdown && (
                      <div className="absolute left-0 top-full mt-0.5 z-10 bg-surface-alt border border-border rounded shadow-lg p-1 max-h-48 overflow-auto">
                        {tableColumns.map((col) => (
                          <label key={col} className="flex items-center gap-1 cursor-pointer text-xs block whitespace-nowrap">
                            <input type="checkbox" checked={visibleTableColumns.includes(col)} onChange={() => toggleTableColumn(col)} />
                            {col}
                          </label>
                        ))}
                        <button type="button" className="mt-1 w-full text-xs rounded bg-surface hover:bg-gray-600" onClick={() => setShowColumnsDropdown(false)}>Close</button>
                      </div>
                    )}
                  </div>
                </>
              )}
            </div>
            <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
              {simViewMode === "chart" ? (
                plotTraces.length > 0 ? (
                  <div className="min-h-full flex flex-col">
                    <Plot
                      data={plotTraces}
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
              ) : simResult ? (
                <div className="min-h-0 flex flex-col flex-1 overflow-hidden">
                  <div className="overflow-auto flex-1 min-h-0 scroll-vscode relative">
                    <table className="w-full text-xs border-collapse">
                      <thead className="sticky top-0 z-20 bg-surface-alt shadow-[0_1px_0_0_var(--border)]">
                        <tr>
                          {(visibleTableColumns.length ? visibleTableColumns : tableColumns).map((col) => (
                            <th
                              key={col}
                              className="border border-border px-2 py-1 text-left cursor-pointer hover:bg-gray-600 bg-surface-alt"
                            onClick={() => {
                              setTableSortKey(col);
                              setTableSortAsc(tableSortKey === col ? !tableSortAsc : true);
                            }}
                          >
                            {col} {tableSortKey === col ? (tableSortAsc ? "\u2191" : "\u2193") : ""}
                          </th>
                        ))}
                        </tr>
                      </thead>
                      <tbody>
                        {paginatedRows.map((row, i) => (
                          <tr key={tablePage * tablePageSize + i}>
                            {(visibleTableColumns.length ? visibleTableColumns : tableColumns).map((col) => (
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
            {logLines.length === 0 ? <div className="text-[var(--text-muted)]">{t("tabLog")}</div> : logLines.filter((line) => !logSearch.trim() || line.includes(logSearch.trim())).map((line, i) => (
              <div key={i} className="text-gray-500">{line}</div>
            ))}
          </div>
        </div>
        )}
      </div>
    </div>
  );
}
