import Plot from "react-plotly.js";
import { t } from "../i18n";
import type { JitValidateResult, SimulationResult } from "../types";

export interface TestAllResultItem {
  path: string;
  success: boolean;
  errors: string[];
}

interface SimulationPanelProps {
  tEnd: number;
  setTEnd: (v: number) => void;
  dt: number;
  setDt: (v: number) => void;
  solver: string;
  setSolver: (v: string) => void;
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
  onExportCSV: () => void;
  onExportJSON: () => void;
  plotTraces: { x: number[]; y: number[]; type: "scatter"; mode: "lines"; name: string }[];
  onSuggestFixWithAi: (msg: string) => void;
}

export function SimulationPanel({
  tEnd,
  setTEnd,
  dt,
  setDt,
  solver,
  setSolver,
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
  onExportCSV,
  onExportJSON,
  plotTraces,
  onSuggestFixWithAi,
}: SimulationPanelProps) {
  return (
    <div className="h-full border-t border-border flex flex-col shrink-0 overflow-hidden bg-surface-alt">
      <div className="flex items-center gap-2 px-2 py-1 border-b border-border flex-wrap">
        <span className="text-xs">t_end</span>
        <input type="number" value={tEnd} onChange={(e) => setTEnd(Number(e.target.value))} className="w-16 bg-[#3c3c3c] border border-gray-600 px-1 text-sm rounded" />
        <span className="text-xs">dt</span>
        <input type="number" step={0.001} value={dt} onChange={(e) => setDt(Number(e.target.value))} className="w-16 bg-[#3c3c3c] border border-gray-600 px-1 text-sm rounded" />
        <span className="text-xs">solver</span>
        <select value={solver} onChange={(e) => setSolver(e.target.value)} className="bg-[#3c3c3c] border border-gray-600 px-1 text-sm rounded">
          <option value="rk4">rk4</option>
          <option value="rk45">rk45</option>
        </select>
        <button type="button" onClick={onValidate} className="px-2 py-1 bg-gray-600 hover:bg-gray-500 text-sm rounded">{t("jitValidate")}</button>
        <button type="button" onClick={onTestAllMoFiles} disabled={testAllLoading || moFilesCount === 0} className="px-2 py-1 bg-gray-600 hover:bg-gray-500 text-sm rounded disabled:opacity-50" title={moFilesCount === 0 ? "" : undefined}>{testAllLoading ? t("testAllRunning") : t("testAllMoFiles")}</button>
        <button type="button" onClick={onRunSimulation} disabled={simLoading} className="px-2 py-1 bg-primary hover:bg-blue-600 text-sm disabled:opacity-50 rounded">{simLoading ? "..." : t("run")}</button>
      </div>
      <div className="flex-1 min-h-0 flex">
        <div className="w-1/2 border-r border-border overflow-auto p-1 text-xs font-mono scroll-vscode">
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
        <div className="w-1/2 min-w-0 flex flex-col">
          <div className="flex items-center gap-2 px-1 py-0.5 border-b border-border shrink-0">
            <button type="button" className={`px-2 py-0.5 text-xs rounded ${simViewMode === "chart" ? "bg-primary text-white" : "bg-surface-alt text-[var(--text-muted)]"}`} onClick={() => setSimViewMode("chart")}>{t("chartView")}</button>
            <button type="button" className={`px-2 py-0.5 text-xs rounded ${simViewMode === "table" ? "bg-primary text-white" : "bg-surface-alt text-[var(--text-muted)]"}`} onClick={() => setSimViewMode("table")}>{t("tableView")}</button>
            {simResult && (
              <>
                <button type="button" className="px-2 py-0.5 text-xs rounded bg-surface-alt text-[var(--text-muted)] hover:bg-gray-600" onClick={onExportCSV}>{t("exportCSV")}</button>
                <button type="button" className="px-2 py-0.5 text-xs rounded bg-surface-alt text-[var(--text-muted)] hover:bg-gray-600" onClick={onExportJSON}>{t("exportJSON")}</button>
              </>
            )}
          </div>
          <div className="flex-1 min-h-0 overflow-auto scroll-vscode">
            {simViewMode === "chart" ? (
              plotTraces.length > 0 ? (
                <Plot
                  data={plotTraces}
                  layout={{
                    margin: { t: 8, r: 8, b: 24, l: 40 },
                    paper_bgcolor: "rgba(0,0,0,0)",
                    plot_bgcolor: "rgba(0,0,0,0)",
                    font: { color: "#d4d4d4", size: 10 },
                    xaxis: { title: "time" },
                    showlegend: true,
                    legend: { x: 1, y: 1 },
                  }}
                  config={{ responsive: true }}
                  style={{ width: "100%", height: "100%" }}
                  useResizeHandler
                />
              ) : (
                <div className="flex items-center justify-center h-full text-[var(--text-muted)] text-sm">{t("runSimulationToSeePlot")}</div>
              )
            ) : simResult ? (
              <table className="w-full text-xs border-collapse">
                <thead className="sticky top-0 bg-surface-alt">
                  <tr>
                    {tableColumns.map((col) => (
                      <th
                        key={col}
                        className="border border-border px-2 py-1 text-left cursor-pointer hover:bg-gray-600"
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
                  {sortedTableRows.slice(0, 500).map((row, i) => (
                    <tr key={i}>
                      {tableColumns.map((col) => (
                        <td key={col} className="border border-border px-2 py-0.5 font-mono">
                          {typeof row[col] === "number" ? (row[col] as number).toExponential(4) : row[col]}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : (
              <div className="flex items-center justify-center h-full text-[var(--text-muted)] text-sm">{t("runSimulationToSeePlot")}</div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
