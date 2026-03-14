import { useEffect, useState } from "react";
import { t } from "../i18n";
import { AppIcon } from "./Icon";
import { IconButton } from "./IconButton";
import { EquationGraphView } from "./EquationGraphView";
import type { JitValidateResult, SimulationResult } from "../types";
import { SimulationRunView } from "./simulation/SimulationRunView";
import type { SimulationChartMeta, SimulationChartSeries } from "./simulation/types";

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
  onClearLog?: () => void;
}

export interface SimResultData {
  jitResult: JitValidateResult | null;
  simResult: SimulationResult | null;
  simLoading: boolean;
  testAllLoading: boolean;
  testAllResults: TestAllResultItem[] | null;
  moFilesCount: number;
  logLines: string[];
  plotSeries: SimulationChartSeries[];
  chartMeta: SimulationChartMeta;
  allPlotVarNames: string[];
  selectedPlotVars: string[];
  tableColumns: string[];
  sortedTableRows: Record<string, number>[];
}

export type BottomTab = "problems" | "output" | "results" | "deps";

const inputClass =
  "w-14 bg-[var(--surface)] border border-border px-1 text-sm rounded text-[var(--text)]";

function pathToModelName(relativePath: string | null | undefined): string {
  if (!relativePath) return "";
  const withoutExt = relativePath.replace(/\.mo$/i, "");
  return withoutExt.replace(/[/\\]/g, ".");
}

interface TabButtonProps {
  active: boolean;
  label: string;
  icon: React.ReactNode;
  badge?: number;
  onClick: () => void;
}

function TabButton({ active, label, icon, badge, onClick }: TabButtonProps) {
  return (
    <button
      type="button"
      className={`relative flex shrink-0 items-center gap-1.5 border-r border-border px-3 py-1.5 text-xs transition-colors ${
        active
          ? "border-b-2 border-b-primary -mb-px bg-surface text-[var(--text)] font-medium"
          : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)]"
      }`}
      onClick={onClick}
    >
      {icon}
      <span>{label}</span>
      {badge != null && badge > 0 && (
        <span className="ml-1 rounded bg-[var(--danger-text)]/20 px-1 text-[10px] font-medium tabular-nums text-[var(--danger-text)]">
          {badge}
        </span>
      )}
    </button>
  );
}

interface SectionHeaderProps {
  title: string;
  expanded: boolean;
  onToggle: () => void;
  statusIcon?: React.ReactNode;
  badge?: React.ReactNode;
  toolbar?: React.ReactNode;
}

function SectionHeader({
  title,
  expanded,
  onToggle,
  statusIcon,
  badge,
  toolbar,
}: SectionHeaderProps) {
  return (
    <div className="flex shrink-0 items-center border-b border-border bg-surface-alt px-2 py-1">
      <button
        type="button"
        className="flex flex-1 items-center gap-1.5 text-left text-[11px] font-semibold uppercase tracking-wide text-[var(--text-muted)]"
        onClick={onToggle}
      >
        <AppIcon
          name="next"
          className={`!h-3 !w-3 transition-transform ${expanded ? "rotate-90" : "rotate-0"}`}
        />
        {statusIcon}
        <span>{title}</span>
        {badge}
      </button>
      {toolbar && (
        <div className="ml-auto flex items-center gap-1">{toolbar}</div>
      )}
    </div>
  );
}

interface SimulationPanelProps {
  params: SimParams;
  onParamChange: <K extends keyof SimParams>(key: K, value: SimParams[K]) => void;
  tableState: SimTableState;
  onTableChange: <K extends keyof SimTableState>(
    key: K,
    value: SimTableState[K]
  ) => void;
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
  const canShowDeps = Boolean(
    code && modelName && openFilePath?.toLowerCase().endsWith(".mo")
  );

  const [showSettings, setShowSettings] = useState(false);
  const [bottomTab, setBottomTab] = useState<BottomTab>("problems");
  const [logSearch, setLogSearch] = useState("");
  const [compilationExpanded, setCompilationExpanded] = useState(true);
  const [variablesExpanded, setVariablesExpanded] = useState(false);
  const [testResultsExpanded, setTestResultsExpanded] = useState(true);

  const togglePlotVar = (name: string) => {
    setSelectedPlotVars((prev) =>
      prev.includes(name) ? prev.filter((v) => v !== name) : [...prev, name]
    );
  };

  useEffect(() => {
    if (!requestedTab) return;
    setBottomTab(requestedTab);
    onRequestedTabHandled?.();
  }, [requestedTab, onRequestedTabHandled]);

  const jitErrorCount =
    data.jitResult && !data.jitResult.success
      ? data.jitResult.errors.length
      : 0;
  const jitWarnCount = data.jitResult?.warnings?.length ?? 0;
  const testFailCount = data.testAllResults
    ? data.testAllResults.filter((r) => !r.success).length
    : 0;
  const problemsBadge = jitErrorCount + testFailCount;
  const totalVarCount =
    (data.jitResult?.state_vars?.length ?? 0) +
    (data.jitResult?.output_vars?.length ?? 0);

  function compilationStatusIcon() {
    if (!data.jitResult) return null;
    if (data.jitResult.success)
      return (
        <AppIcon
          name="validate"
          className="!h-3.5 !w-3.5 text-[var(--success-text)]"
        />
      );
    return (
      <AppIcon
        name="error"
        className="!h-3.5 !w-3.5 text-[var(--danger-text)]"
      />
    );
  }

  function renderStatusBadge() {
    if (data.simLoading || data.testAllLoading) {
      return (
        <div className="flex items-center gap-1.5 text-xs text-[var(--text-muted)]">
          <AppIcon name="spinner" className="!h-3.5 !w-3.5 animate-spin" />
          <span>{t("running")}</span>
        </div>
      );
    }
    if (data.jitResult) {
      if (data.jitResult.success) {
        return (
          <div className="flex items-center gap-1.5 text-xs text-[var(--success-text)]">
            <span className="h-2 w-2 rounded-full bg-[var(--success-text)]" />
            <span>{t("jitStatusOk")}</span>
          </div>
        );
      }
      return (
        <div className="flex items-center gap-1.5 text-xs text-[var(--danger-text)]">
          <span className="h-2 w-2 rounded-full bg-[var(--danger-text)]" />
          <span>
            {jitErrorCount} {t("jitStatusErrors")}
          </span>
        </div>
      );
    }
    return null;
  }

  function buildTestAllSummary() {
    if (!data.testAllResults) return null;
    const passed = data.testAllResults.filter((r) => r.success).length;
    const failed = data.testAllResults.filter((r) => !r.success).length;
    const lines: string[] = [];
    for (const r of data.testAllResults) {
      if (r.success) {
        lines.push("PASS " + r.path);
      } else {
        lines.push("FAIL " + r.path);
        for (const err of r.errors) lines.push("  " + err);
      }
    }
    lines.push("---");
    lines.push(`Summary: ${passed} passed, ${failed} failed`);
    return { text: lines.join("\n"), passed, failed };
  }

  const testSummary = buildTestAllSummary();

  return (
    <div className="h-full border-t border-border flex flex-col shrink-0 overflow-hidden bg-surface-alt">
      {/* Action bar */}
      <div className="border-b border-border flex items-center gap-1 px-2 py-1 shrink-0">
        <IconButton
          icon={<AppIcon name="validate" aria-hidden="true" />}
          onClick={actions.onValidate}
          title={t("jitValidate")}
          aria-label={t("jitValidate")}
        />
        <IconButton
          icon={<AppIcon name="run" aria-hidden="true" />}
          variant="primary"
          onClick={actions.onRunSimulation}
          disabled={data.simLoading}
          title={t("run")}
          aria-label={t("run")}
        />
        <div className="mx-1 h-5 w-px bg-border" />
        <IconButton
          icon={<AppIcon name="gitCommit" aria-hidden="true" />}
          size="xs"
          onClick={actions.onTestAll}
          disabled={data.testAllLoading || data.moFilesCount === 0}
          title={t("testAllMoFiles")}
          aria-label={t("testAllMoFiles")}
        />
        <div className="flex-1 min-w-0" />
        {renderStatusBadge()}
        <div className="ml-2">
          <IconButton
            icon={<AppIcon name="simSettings" aria-hidden="true" />}
            active={showSettings}
            onClick={() => setShowSettings((s) => !s)}
            title={t("simSettings")}
            aria-label={t("simSettings")}
          />
        </div>
      </div>

      {/* Settings panel (inline collapsible) */}
      {showSettings && (
        <div className="shrink-0 border-b border-border bg-surface px-2 py-2">
          <div className="flex flex-wrap gap-x-4 gap-y-2 text-xs">
            <fieldset className="flex flex-wrap items-center gap-2 rounded border border-border px-2 py-1">
              <legend className="text-[var(--text-muted)]">
                {t("simGroupSimulation")}
              </legend>
              <label className="flex items-center gap-1">
                <span>{t("paramTEnd")}</span>
                <input
                  type="number"
                  value={params.tEnd}
                  onChange={(e) =>
                    onParamChange("tEnd", Number(e.target.value))
                  }
                  className={inputClass}
                />
              </label>
              <label className="flex items-center gap-1">
                <span>{t("paramDt")}</span>
                <input
                  type="number"
                  step={0.001}
                  value={params.dt}
                  onChange={(e) => onParamChange("dt", Number(e.target.value))}
                  className={inputClass}
                />
              </label>
              <label className="flex items-center gap-1">
                <span>{t("paramSolver")}</span>
                <select
                  value={params.solver}
                  onChange={(e) => onParamChange("solver", e.target.value)}
                  className={inputClass}
                >
                  <option value="rk4">rk4</option>
                  <option value="rk45">rk45</option>
                  <option value="implicit">{t("implicitSolver")}</option>
                </select>
              </label>
              <label className="flex items-center gap-1">
                <span>{t("paramOutputInterval")}</span>
                <input
                  type="number"
                  step={0.001}
                  value={params.outputInterval}
                  onChange={(e) =>
                    onParamChange("outputInterval", Number(e.target.value))
                  }
                  className={inputClass}
                />
              </label>
            </fieldset>
            <fieldset className="flex flex-wrap items-center gap-2 rounded border border-border px-2 py-1">
              <legend className="text-[var(--text-muted)]">
                {t("simGroupTolerance")}
              </legend>
              <label className="flex items-center gap-1">
                <span>{t("paramAtol")}</span>
                <input
                  type="number"
                  step={1e-12}
                  value={params.atol}
                  onChange={(e) =>
                    onParamChange("atol", Number(e.target.value))
                  }
                  className={inputClass}
                />
              </label>
              <label className="flex items-center gap-1">
                <span>{t("paramRtol")}</span>
                <input
                  type="number"
                  step={1e-6}
                  value={params.rtol}
                  onChange={(e) =>
                    onParamChange("rtol", Number(e.target.value))
                  }
                  className={inputClass}
                />
              </label>
            </fieldset>
          </div>
        </div>
      )}

      {/* Tab bar */}
      <div className="flex shrink-0 items-stretch border-b border-border bg-surface-alt">
        <TabButton
          active={bottomTab === "problems"}
          label={t("tabProblems")}
          icon={
            <AppIcon
              name="warning"
              className="!h-3.5 !w-3.5"
              aria-hidden="true"
            />
          }
          badge={problemsBadge > 0 ? problemsBadge : undefined}
          onClick={() => setBottomTab("problems")}
        />
        <TabButton
          active={bottomTab === "output"}
          label={t("tabOutput")}
          icon={
            <AppIcon
              name="output"
              className="!h-3.5 !w-3.5"
              aria-hidden="true"
            />
          }
          onClick={() => setBottomTab("output")}
        />
        <TabButton
          active={bottomTab === "results"}
          label={t("tabResults")}
          icon={
            <AppIcon
              name="chart"
              className="!h-3.5 !w-3.5"
              aria-hidden="true"
            />
          }
          onClick={() => setBottomTab("results")}
        />
        {canShowDeps && (
          <TabButton
            active={bottomTab === "deps"}
            label={t("tabDependencies")}
            icon={
              <AppIcon
                name="link"
                className="!h-3.5 !w-3.5"
                aria-hidden="true"
              />
            }
            onClick={() => setBottomTab("deps")}
          />
        )}
      </div>

      {/* Tab content */}
      <div className="flex flex-1 min-h-0 overflow-hidden">
        {/* Problems tab */}
        {bottomTab === "problems" && (
          <div className="flex flex-1 min-h-0 flex-col overflow-hidden">
            <div className="flex-1 overflow-auto scroll-vscode">
              {/* Compilation section */}
              <SectionHeader
                title={t("sectionCompilation")}
                expanded={compilationExpanded}
                onToggle={() => setCompilationExpanded((v) => !v)}
                statusIcon={compilationStatusIcon()}
                badge={
                  jitErrorCount > 0 ? (
                    <span className="ml-1 rounded bg-[var(--danger-text)]/15 px-1.5 text-[10px] text-[var(--danger-text)]">
                      {jitErrorCount}
                    </span>
                  ) : jitWarnCount > 0 ? (
                    <span className="ml-1 rounded bg-[var(--warning-text)]/15 px-1.5 text-[10px] text-[var(--warning-text)]">
                      {jitWarnCount}
                    </span>
                  ) : undefined
                }
              />
              {compilationExpanded && (
                <div className="px-3 py-2 text-xs font-mono">
                  {!data.jitResult && (
                    <div className="italic text-[var(--text-muted)]">
                      {t("jitStatusNotRun")}
                    </div>
                  )}
                  {data.jitResult?.success && (
                    <div className="flex items-center gap-2">
                      <AppIcon
                        name="validate"
                        className="!h-3.5 !w-3.5 shrink-0 text-[var(--success-text)]"
                      />
                      <span className="text-[var(--success-text)]">
                        {t("jitStatusOk")}
                      </span>
                      {totalVarCount > 0 && (
                        <span className="text-[var(--text-muted)]">
                          &mdash; {totalVarCount}{" "}
                          {t("variablesSelect").toLowerCase()}
                        </span>
                      )}
                    </div>
                  )}
                  {data.jitResult && !data.jitResult.success && (
                    <div className="space-y-1">
                      {data.jitResult.errors.map((e, i) => (
                        <div key={i} className="text-[var(--danger-text)]">
                          {e}
                        </div>
                      ))}
                      <button
                        type="button"
                        onClick={() =>
                          actions.onSuggestFixWithAi(
                            "Fix the following Modelica compile error and suggest corrected code: " +
                              data.jitResult!.errors.join(" ")
                          )
                        }
                        className="mt-2 rounded bg-primary/80 px-2 py-0.5 text-xs text-white hover:bg-primary"
                      >
                        {t("suggestFixWithAi")}
                      </button>
                    </div>
                  )}
                  {data.jitResult?.warnings && data.jitResult.warnings.length > 0 && (
                    <div
                      className={`space-y-0.5 ${data.jitResult.success ? "mt-2" : "mt-1"}`}
                    >
                      {data.jitResult.warnings.map((w, i) => (
                        <div key={i} className="text-[var(--warning-text)]">
                          {w.path}:{w.line}:{w.column} {w.message}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}

              {/* Variables section (only when jitResult present) */}
              {data.jitResult && (
                <>
                  <SectionHeader
                    title={t("sectionVariables")}
                    expanded={variablesExpanded}
                    onToggle={() => setVariablesExpanded((v) => !v)}
                    badge={
                      totalVarCount > 0 ? (
                        <span className="ml-1 rounded bg-[var(--text-muted)]/15 px-1.5 text-[10px] text-[var(--text-muted)]">
                          {totalVarCount}
                        </span>
                      ) : undefined
                    }
                  />
                  {variablesExpanded && (
                    <div className="space-y-3 px-3 py-2 text-xs">
                      {(data.jitResult.state_vars?.length ?? 0) > 0 && (
                        <div>
                          <div className="mb-1 text-[10px] uppercase tracking-wide text-[var(--text-muted)]">
                            state
                          </div>
                          <div className="space-y-0.5">
                            {data.jitResult.state_vars.map((name) => (
                              <button
                                key={`state:${name}`}
                                type="button"
                                className={`block w-full rounded px-2 py-0.5 text-left font-mono text-[11px] ${
                                  selectedSymbol === name
                                    ? "bg-primary/20 text-primary"
                                    : "text-[var(--text)] hover:bg-white/5"
                                }`}
                                onClick={() => onFocusSymbol?.(name)}
                              >
                                {name}
                              </button>
                            ))}
                          </div>
                        </div>
                      )}
                      {(data.jitResult.output_vars?.length ?? 0) > 0 && (
                        <div>
                          <div className="mb-1 text-[10px] uppercase tracking-wide text-[var(--text-muted)]">
                            output
                          </div>
                          <div className="space-y-0.5">
                            {data.jitResult.output_vars.map((name) => (
                              <button
                                key={`output:${name}`}
                                type="button"
                                className={`block w-full rounded px-2 py-0.5 text-left font-mono text-[11px] ${
                                  selectedSymbol === name
                                    ? "bg-primary/20 text-primary"
                                    : "text-[var(--text)] hover:bg-white/5"
                                }`}
                                onClick={() => onFocusSymbol?.(name)}
                              >
                                {name}
                              </button>
                            ))}
                          </div>
                        </div>
                      )}
                      {totalVarCount === 0 && (
                        <div className="text-[11px] italic text-[var(--text-muted)]">
                          {t("runJitFirst")}
                        </div>
                      )}
                    </div>
                  )}
                </>
              )}

              {/* Test results section (only when testAllResults present) */}
              {data.testAllResults !== null && testSummary && (
                <>
                  <SectionHeader
                    title={t("sectionTestResults")}
                    expanded={testResultsExpanded}
                    onToggle={() => setTestResultsExpanded((v) => !v)}
                    badge={
                      testSummary.failed > 0 ? (
                        <span className="ml-1 rounded bg-[var(--danger-text)]/15 px-1.5 text-[10px] text-[var(--danger-text)]">
                          {testSummary.failed} failed
                        </span>
                      ) : (
                        <span className="ml-1 rounded bg-[var(--success-text)]/15 px-1.5 text-[10px] text-[var(--success-text)]">
                          {testSummary.passed} passed
                        </span>
                      )
                    }
                    toolbar={
                      <button
                        type="button"
                        className="rounded border border-border px-1.5 py-0.5 text-[10px] theme-button-secondary"
                        onClick={() =>
                          void navigator.clipboard.writeText(testSummary.text)
                        }
                      >
                        {t("copyTestAllOutput")}
                      </button>
                    }
                  />
                  {testResultsExpanded && (
                    <div className="space-y-0.5 px-3 py-2 text-xs">
                      {data.testAllResults.map((r, i) => (
                        <div
                          key={i}
                          className={
                            r.success
                              ? "text-[var(--success-text)]"
                              : "text-[var(--danger-text)]"
                          }
                        >
                          {r.success ? "\u2713" : "\u2717"} {r.path}
                          {!r.success && r.errors.length > 0 && (
                            <div className="pl-3 font-mono text-[11px] text-[var(--warning-text)]">
                              {r.errors[0]}
                            </div>
                          )}
                        </div>
                      ))}
                    </div>
                  )}
                </>
              )}

              {/* Empty state when nothing has run */}
              {!data.jitResult && data.testAllResults === null && (
                <div className="px-3 py-10 text-center text-xs text-[var(--text-muted)]">
                  {t("jitStatusNotRun")}
                </div>
              )}
            </div>
          </div>
        )}

        {/* Output tab */}
        {bottomTab === "output" && (
          <div className="flex flex-1 min-h-0 flex-col">
            <div className="flex shrink-0 items-center gap-2 border-b border-border px-2 py-1">
              <input
                type="text"
                placeholder={t("tableSearch")}
                value={logSearch}
                onChange={(e) => setLogSearch(e.target.value)}
                className="max-w-xs flex-1 rounded border border-border bg-surface px-2 py-0.5 text-xs"
              />
              <button
                type="button"
                className="ml-auto rounded border border-border px-2 py-0.5 text-xs theme-button-secondary"
                onClick={actions.onClearLog}
                disabled={!actions.onClearLog || data.logLines.length === 0}
              >
                {t("clearLog")}
              </button>
            </div>
            <div className="flex-1 overflow-auto p-2 font-mono text-xs scroll-vscode">
              {data.logLines.length === 0 ? (
                <div className="text-[var(--text-muted)]">{t("tabOutput")}</div>
              ) : (
                data.logLines
                  .filter(
                    (line) =>
                      !logSearch.trim() ||
                      line
                        .toLowerCase()
                        .includes(logSearch.trim().toLowerCase())
                  )
                  .map((line, i) => (
                    <div
                      key={i}
                      className="py-0.5 leading-tight text-[var(--text-muted)]"
                    >
                      {line}
                    </div>
                  ))
              )}
            </div>
          </div>
        )}

        {/* Results tab */}
        {bottomTab === "results" && (
          <div className="flex min-h-0 min-w-0 flex-1">
            <SimulationRunView
              theme={theme}
              simResult={data.simResult}
              timeValues={data.simResult?.time ?? []}
              plotSeries={data.plotSeries}
              chartMeta={data.chartMeta}
              allPlotVarNames={data.allPlotVarNames}
              selectedPlotVars={data.selectedPlotVars}
              tableSortKey={tableState.tableSortKey}
              tableSortAsc={tableState.tableSortAsc}
              tablePage={tableState.tablePage}
              tablePageSize={tableState.tablePageSize}
              visibleTableColumns={tableState.visibleTableColumns}
              tableColumns={data.tableColumns}
              sortedTableRows={data.sortedTableRows}
              simViewMode={tableState.simViewMode}
              onViewModeChange={(mode) => onTableChange("simViewMode", mode)}
              onSortKeyChange={(value) => onTableChange("tableSortKey", value)}
              onSortAscChange={(value) => onTableChange("tableSortAsc", value)}
              onPageChange={(value) => onTableChange("tablePage", value)}
              onPageSizeChange={(value) =>
                onTableChange("tablePageSize", value)
              }
              onVisibleColumnsChange={(value) =>
                onTableChange("visibleTableColumns", value)
              }
              onSelectPlotVars={(value) => setSelectedPlotVars(value)}
              onTogglePlotVar={togglePlotVar}
              onExportCSV={actions.onExportCSV}
              onExportJSON={actions.onExportJSON}
            />
          </div>
        )}

        {/* Dependencies tab */}
        {bottomTab === "deps" && canShowDeps && (
          <div className="flex flex-1 min-h-0 flex-col">
            <EquationGraphView
              code={code}
              modelName={modelName}
              projectDir={projectDir}
            />
          </div>
        )}
      </div>
    </div>
  );
}
