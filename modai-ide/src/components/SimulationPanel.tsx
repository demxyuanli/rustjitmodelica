import { useCallback, useEffect, useMemo, useState } from "react";
import { suggestLibraryForMissingType, installThirdPartyLibraryFromGit } from "../api/tauri";
import type { AppSettings } from "../api/tauri";
import { dependencyGraphBehaviorFromAppSettings } from "../utils/dependencyGraphBehavior";
import { t } from "../i18n";
import type { JointPaperHandle } from "../utils/jointUtils";
import { AppIcon } from "./Icon";
import { IconButton } from "./IconButton";
import type { JitValidateResult, LibrarySuggestion, SimulationResult } from "../types";
import type {
  SimulationChartMeta,
  SimulationChartSeries,
  TestAllResultItem,
} from "./simulation/types";
import { ContextMenu } from "./ContextMenu";
import { SimulationBottomTabBar } from "./simulation/SimulationBottomTabBar";
import { SimulationDependenciesTab } from "./simulation/SimulationDependenciesTab";
import { SimulationOutputTab } from "./simulation/SimulationOutputTab";
import { SimulationProblemsTab } from "./simulation/SimulationProblemsTab";
import { SimulationResultsTab } from "./simulation/SimulationResultsTab";
import type { BottomTab } from "./simulation/simulationBottomTabTypes";
import {
  parseUnknownTypeFromError,
  pathToModelName,
  simulationInputClass,
} from "./simulation/simulationPanelUtils";

export type { BottomTab, TestAllResultItem };

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
  /** Raw message strings (no time prefix); parent adds timestamps like live log. */
  onAppendLogLines?: (lines: string[]) => void;
}

export interface SimResultData {
  jitResult: JitValidateResult | null;
  simResult: SimulationResult | null;
  validateLoading: boolean;
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
  appSettings?: AppSettings | null;
  onOpenDependencyGraphSettings?: () => void;
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
  appSettings = null,
  onOpenDependencyGraphSettings,
}: SimulationPanelProps) {
  const dependencyGraphBehavior = useMemo(
    () => dependencyGraphBehaviorFromAppSettings(appSettings),
    [appSettings]
  );
  const modelName = pathToModelName(openFilePath);
  const canShowDeps = Boolean(
    code && modelName && openFilePath?.toLowerCase().endsWith(".mo")
  );

  useEffect(() => {
    const errors = data.jitResult?.errors ?? [];
    setSuggestionByIndex({});
    setInstallMessage(null);
    errors.forEach((err, i) => {
      const typeName = parseUnknownTypeFromError(err);
      if (typeName) {
        void suggestLibraryForMissingType(typeName).then((s) => {
          if (s) setSuggestionByIndex((prev) => ({ ...prev, [i]: s }));
        });
      }
    });
  }, [data.jitResult?.errors]);

  const handleInstallSuggestedLibrary = useCallback(
    async (suggestion: LibrarySuggestion) => {
      setInstallBusy(true);
      setInstallMessage(null);
      try {
        await installThirdPartyLibraryFromGit({
          projectDir: projectDir ?? undefined,
          scope: "global",
          url: suggestion.url,
          refName: suggestion.refName,
          displayName: suggestion.displayName,
        });
        setInstallMessage(t("componentLibraryInstalledFromUrl") + " " + t("jitValidate") + ".");
      } catch (e) {
        setInstallMessage(String(e));
      } finally {
        setInstallBusy(false);
      }
    },
    [projectDir]
  );

  const [showSettings, setShowSettings] = useState(false);
  const [dependencyGraphModalOpen, setDependencyGraphModalOpen] = useState(false);
  const [suggestionByIndex, setSuggestionByIndex] = useState<Record<number, LibrarySuggestion>>({});
  const [installBusy, setInstallBusy] = useState(false);
  const [installMessage, setInstallMessage] = useState<string | null>(null);
  const [depPaperHandle, setDepPaperHandle] = useState<JointPaperHandle | null>(null);
  const handleDepZoomIn = useCallback(() => depPaperHandle?.zoomIn(), [depPaperHandle]);
  const handleDepZoomOut = useCallback(() => depPaperHandle?.zoomOut(), [depPaperHandle]);
  const handleDepFitView = useCallback(() => depPaperHandle?.fitView(), [depPaperHandle]);
  const [bottomTab, setBottomTab] = useState<BottomTab>("problems");
  const [logSearch, setLogSearch] = useState("");
  const [compilationExpanded, setCompilationExpanded] = useState(true);
  const [variablesExpanded, setVariablesExpanded] = useState(false);
  const [testResultsExpanded, setTestResultsExpanded] = useState(true);
  const [outputMenuVisible, setOutputMenuVisible] = useState(false);
  const [outputMenuPosition, setOutputMenuPosition] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const [errorMenuVisible, setErrorMenuVisible] = useState(false);
  const [errorMenuPosition, setErrorMenuPosition] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const [errorMenuText, setErrorMenuText] = useState<string | null>(null);

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

  function renderStatusBadge() {
    if (data.validateLoading || data.simLoading || data.testAllLoading) {
      return (
        <div className="flex items-center gap-1.5 text-xs text-[var(--text-muted)]">
          <AppIcon name="spinner" className="!h-3.5 !w-3.5 animate-spin" />
          <span>
            {data.validateLoading ? t("jitValidate")
            : data.simLoading ? t("run")
            : t("testAllMoFiles")} {t("running")}
          </span>
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
          disabled={data.validateLoading || data.simLoading}
          title={t("jitValidate")}
          aria-label={t("jitValidate")}
        />
        <IconButton
          icon={<AppIcon name="run" aria-hidden="true" />}
          variant="primary"
          onClick={actions.onRunSimulation}
          disabled={data.simLoading || data.validateLoading}
          title={t("run")}
          aria-label={t("run")}
        />
        <div className="mx-1 h-5 w-px bg-border" />
        <IconButton
          icon={<AppIcon name="gitCommit" aria-hidden="true" />}
          size="xs"
          onClick={actions.onTestAll}
          disabled={data.testAllLoading || data.simLoading || data.validateLoading || data.moFilesCount === 0}
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
                  className={simulationInputClass}
                />
              </label>
              <label className="flex items-center gap-1">
                <span>{t("paramDt")}</span>
                <input
                  type="number"
                  step={0.001}
                  value={params.dt}
                  onChange={(e) => onParamChange("dt", Number(e.target.value))}
                  className={simulationInputClass}
                />
              </label>
              <label className="flex items-center gap-1">
                <span>{t("paramSolver")}</span>
                <select
                  value={params.solver}
                  onChange={(e) => onParamChange("solver", e.target.value)}
                  className={simulationInputClass}
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
                  className={simulationInputClass}
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
                  className={simulationInputClass}
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
                  className={simulationInputClass}
                />
              </label>
            </fieldset>
          </div>
        </div>
      )}

      <SimulationBottomTabBar
        bottomTab={bottomTab}
        onBottomTabChange={setBottomTab}
        canShowDeps={canShowDeps}
        problemsBadge={problemsBadge}
      />

      <div className="flex flex-1 min-h-0 overflow-hidden">
        {bottomTab === "problems" && (
          <SimulationProblemsTab
            jitResult={data.jitResult}
            testAllResults={data.testAllResults}
            testSummary={testSummary}
            onSuggestFixWithAi={actions.onSuggestFixWithAi}
            compilationExpanded={compilationExpanded}
            onCompilationExpandedToggle={() =>
              setCompilationExpanded((v) => !v)
            }
            variablesExpanded={variablesExpanded}
            onVariablesExpandedToggle={() => setVariablesExpanded((v) => !v)}
            testResultsExpanded={testResultsExpanded}
            onTestResultsExpandedToggle={() =>
              setTestResultsExpanded((v) => !v)
            }
            suggestionByIndex={suggestionByIndex}
            installBusy={installBusy}
            installMessage={installMessage}
            onInstallSuggestedLibrary={handleInstallSuggestedLibrary}
            selectedSymbol={selectedSymbol}
            onFocusSymbol={onFocusSymbol}
            onErrorContextMenu={(x, y, text) => {
              setErrorMenuPosition({ x, y });
              setErrorMenuText(text);
              setErrorMenuVisible(true);
            }}
            jitErrorCount={jitErrorCount}
            jitWarnCount={jitWarnCount}
            totalVarCount={totalVarCount}
          />
        )}

        {bottomTab === "output" && (
          <SimulationOutputTab
            logSearch={logSearch}
            onLogSearchChange={setLogSearch}
            logLines={data.logLines}
            onClearLog={actions.onClearLog}
            onAppendLogLines={actions.onAppendLogLines}
            onOutputContextMenu={(x, y) => {
              setOutputMenuPosition({ x, y });
              setOutputMenuVisible(true);
            }}
          />
        )}

        {bottomTab === "results" && (
          <SimulationResultsTab
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
            onPageSizeChange={(value) => onTableChange("tablePageSize", value)}
            onVisibleColumnsChange={(value) =>
              onTableChange("visibleTableColumns", value)
            }
            onSelectPlotVars={(value) => setSelectedPlotVars(value)}
            onTogglePlotVar={togglePlotVar}
            onExportCSV={actions.onExportCSV}
            onExportJSON={actions.onExportJSON}
          />
        )}

        {bottomTab === "deps" && canShowDeps && (
          <SimulationDependenciesTab
            code={code}
            modelName={modelName ?? ""}
            projectDir={projectDir}
            dependencyGraphBehavior={dependencyGraphBehavior}
            onZoomIn={handleDepZoomIn}
            onZoomOut={handleDepZoomOut}
            onFitView={handleDepFitView}
            dependencyGraphModalOpen={dependencyGraphModalOpen}
            setDependencyGraphModalOpen={setDependencyGraphModalOpen}
            onDepPaperReady={setDepPaperHandle}
            onOpenDependencyGraphSettings={onOpenDependencyGraphSettings}
          />
        )}
      </div>
      <ContextMenu
        visible={outputMenuVisible}
        x={outputMenuPosition.x}
        y={outputMenuPosition.y}
        onClose={() => setOutputMenuVisible(false)}
        items={[
          {
            id: "copy-all",
            label: t("copyTestAllOutput"),
            onClick: () => {
              if (data.logLines.length === 0) return;
              void navigator.clipboard.writeText(data.logLines.join("\n"));
            },
          },
          {
            id: "clear-log",
            label: t("clearLog"),
            onClick: () => {
              actions.onClearLog?.();
            },
          },
        ]}
      />
      <ContextMenu
        visible={errorMenuVisible}
        x={errorMenuPosition.x}
        y={errorMenuPosition.y}
        onClose={() => setErrorMenuVisible(false)}
        items={
          errorMenuText
            ? [
                {
                  id: "copy-error",
                  label: t("contextCopyError"),
                  onClick: () => {
                    void navigator.clipboard.writeText(errorMenuText);
                  },
                },
                {
                  id: "ai-fix-error",
                  label: t("suggestFixWithAi"),
                  onClick: () => {
                    actions.onSuggestFixWithAi(
                      "Fix the following Modelica compile error and suggest corrected code: " +
                        errorMenuText
                    );
                  },
                },
              ]
            : []
        }
      />
    </div>
  );
}
