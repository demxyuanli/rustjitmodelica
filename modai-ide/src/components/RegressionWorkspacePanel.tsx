import { useCallback, useEffect, useMemo, useState } from "react";
import {
  regressionCancelWorkspace,
  regressionCreateWorkspace,
  regressionGetWorkspaceState,
  regressionListWorkspaces,
  regressionRunWorkspace,
} from "../api/tauri";
import type {
  RegressionPlanRequest,
  RegressionPlanStrategy,
  RegressionWorkspaceInfo,
  RegressionWorkspaceState,
} from "../types";
import { t } from "../i18n";

const CATEGORY_OPTIONS = [
  "basic",
  "initialization",
  "array",
  "connect",
  "discrete",
  "algebraic",
  "solver",
  "function",
  "structure",
  "msl",
  "tooling",
  "error",
];

function pct(a: number, b: number): string {
  if (!b) return "0.0%";
  return `${((a / b) * 100).toFixed(1)}%`;
}

function statusTone(status: string): string {
  if (status === "completed") return "theme-banner-success";
  if (status === "failed" || status === "cancelled") return "theme-banner-danger";
  if (status === "running") return "theme-banner-warning";
  return "theme-button-secondary";
}

export function RegressionWorkspacePanel({ theme: _theme = "dark" }: { theme?: "dark" | "light" }) {
  const [strategy, setStrategy] = useState<RegressionPlanStrategy>("relation");
  const [categories, setCategories] = useState<string[]>([]);
  const [featureIdsRaw, setFeatureIdsRaw] = useState("");
  const [changedFilesRaw, setChangedFilesRaw] = useState("");
  const [includeIndirect, setIncludeIndirect] = useState(true);
  const [maxCasesRaw, setMaxCasesRaw] = useState("");
  const [includeModelicaExamples, setIncludeModelicaExamples] = useState(true);
  const [includeModelicaTest, setIncludeModelicaTest] = useState(true);
  const [workspaceMode, setWorkspaceMode] = useState<"persistent" | "ephemeral">("persistent");

  const [workspaces, setWorkspaces] = useState<RegressionWorkspaceInfo[]>([]);
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState<string | null>(null);
  const [state, setState] = useState<RegressionWorkspaceState | null>(null);
  const [loading, setLoading] = useState(false);
  const [running, setRunning] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<"monitor" | "stats" | "records">("monitor");
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [showFailedOnly, setShowFailedOnly] = useState(false);

  const refreshList = useCallback(async () => {
    const list = await regressionListWorkspaces();
    setWorkspaces(list);
    if (!selectedWorkspaceId && list.length > 0) setSelectedWorkspaceId(list[0].workspaceId);
  }, [selectedWorkspaceId]);

  const refreshState = useCallback(
    async (workspaceId?: string | null) => {
      const id = workspaceId ?? selectedWorkspaceId;
      if (!id) return;
      const next = await regressionGetWorkspaceState(id);
      setState(next);
    },
    [selectedWorkspaceId]
  );

  useEffect(() => {
    refreshList().catch((e) => setMessage(String(e)));
  }, [refreshList]);

  useEffect(() => {
    if (selectedWorkspaceId) {
      refreshState(selectedWorkspaceId).catch((e) => setMessage(String(e)));
    }
  }, [selectedWorkspaceId, refreshState]);

  useEffect(() => {
    if (!autoRefresh || !selectedWorkspaceId) return;
    const timer = window.setInterval(() => {
      refreshState(selectedWorkspaceId).catch(() => {});
      refreshList().catch(() => {});
    }, 3000);
    return () => window.clearInterval(timer);
  }, [autoRefresh, selectedWorkspaceId, refreshState, refreshList]);

  const summary = useMemo(() => {
    if (!state?.records || state.records.length === 0) {
      return {
        total: 0,
        passed: 0,
        failed: 0,
        failByReason: [] as Array<{ key: string; count: number }>,
        passByCategory: [] as Array<{ key: string; count: number }>,
      };
    }
    const failReasonMap = new Map<string, number>();
    const passCategoryMap = new Map<string, number>();
    let passed = 0;
    let failed = 0;
    for (const r of state.records) {
      if (r.actualOk) {
        passed += 1;
        const cat = r.detail.split("category=").pop() ?? "unknown";
        passCategoryMap.set(cat, (passCategoryMap.get(cat) ?? 0) + 1);
      } else {
        failed += 1;
        failReasonMap.set(r.reason, (failReasonMap.get(r.reason) ?? 0) + 1);
      }
    }
    return {
      total: state.records.length,
      passed,
      failed,
      failByReason: Array.from(failReasonMap.entries())
        .map(([key, count]) => ({ key, count }))
        .sort((a, b) => b.count - a.count),
      passByCategory: Array.from(passCategoryMap.entries())
        .map(([key, count]) => ({ key, count }))
        .sort((a, b) => b.count - a.count),
    };
  }, [state]);

  const visibleRecords = useMemo(() => {
    if (!state) return [];
    if (!showFailedOnly) return state.records;
    return state.records.filter((x) => !x.actualOk);
  }, [state, showFailedOnly]);

  const createPlan = useCallback(async () => {
    setLoading(true);
    setMessage(null);
    try {
      const req: RegressionPlanRequest = {
        strategy,
        categories,
        featureIds: featureIdsRaw
          .split(/[,\r\n]/)
          .map((s) => s.trim())
          .filter(Boolean),
        changedFiles: changedFilesRaw
          .split(/[\r\n]/)
          .map((s) => s.trim())
          .filter(Boolean),
        includeIndirect,
        maxCases: maxCasesRaw.trim() ? Math.max(1, Number(maxCasesRaw)) : null,
        workspaceMode,
        includeModelicaExamples,
        includeModelicaTest,
      };
      const created = await regressionCreateWorkspace(req);
      setSelectedWorkspaceId(created.info.workspaceId);
      setState(created);
      await refreshList();
      setMessage(`${t("regressionCreated")} ${created.info.workspaceId}`);
    } catch (e) {
      setMessage(String(e));
    } finally {
      setLoading(false);
    }
  }, [
    strategy,
    categories,
    featureIdsRaw,
    changedFilesRaw,
    includeIndirect,
    maxCasesRaw,
    workspaceMode,
    includeModelicaExamples,
    includeModelicaTest,
    refreshList,
  ]);

  const runCurrent = useCallback(async () => {
    if (!selectedWorkspaceId) return;
    setRunning(true);
    setMessage(null);
    try {
      const next = await regressionRunWorkspace(selectedWorkspaceId);
      setState(next);
      await refreshList();
      setMessage(`${t("regressionRunFinished")} ${next.info.status}`);
    } catch (e) {
      setMessage(String(e));
    } finally {
      setRunning(false);
    }
  }, [selectedWorkspaceId, refreshList]);

  const cancelCurrent = useCallback(async () => {
    if (!selectedWorkspaceId) return;
    setMessage(null);
    try {
      const next = await regressionCancelWorkspace(selectedWorkspaceId);
      setState(next);
      await refreshList();
      setMessage(`${t("regressionCancelled")} ${selectedWorkspaceId}`);
    } catch (e) {
      setMessage(String(e));
    }
  }, [selectedWorkspaceId, refreshList]);

  return (
    <div className="flex h-full min-h-0 w-full flex-col overflow-hidden bg-surface">
      <div className="panel-header-min-height shrink-0 border-b border-border bg-[var(--surface-elevated)] px-3 flex items-center justify-between gap-3">
        <div className="min-w-0">
          <div className="text-xs uppercase text-[var(--text-muted)]">{t("workspaceRegression")}</div>
          <div className="text-[11px] text-[var(--text-muted)] truncate">{t("testManagerDesc")}</div>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <select
            value={selectedWorkspaceId ?? ""}
            onChange={(e) => setSelectedWorkspaceId(e.target.value || null)}
            className="theme-input border px-2 py-1 text-xs rounded w-72"
          >
            <option value="">{t("regressionSelectWorkspace")}</option>
            {workspaces.map((w) => (
              <option key={w.workspaceId} value={w.workspaceId}>
                {w.workspaceId} [{w.status}]
              </option>
            ))}
          </select>
          <button
            type="button"
            onClick={runCurrent}
            disabled={!selectedWorkspaceId || running}
            className="px-2.5 py-1 text-xs rounded border theme-banner-success disabled:opacity-50"
          >
            {running ? t("running") : t("run")}
          </button>
          <button
            type="button"
            onClick={() => refreshState().catch((e) => setMessage(String(e)))}
            disabled={!selectedWorkspaceId}
            className="px-2.5 py-1 text-xs rounded border theme-button-secondary disabled:opacity-50"
          >
            {t("refresh")}
          </button>
          <button
            type="button"
            onClick={cancelCurrent}
            disabled={!selectedWorkspaceId}
            className="px-2.5 py-1 text-xs rounded border theme-banner-danger disabled:opacity-50"
          >
            {t("cancel")}
          </button>
        </div>
      </div>

      <div className="flex-1 min-h-0 overflow-hidden grid grid-cols-12">
      <aside className="col-span-4 min-w-0 border-r border-border bg-[var(--panel-bg)] p-3 overflow-auto">
        <div className="text-xs uppercase text-[var(--text-muted)] mb-2">{t("regressionPlan")}</div>

        <label className="text-xs text-[var(--text-muted)]">{t("regressionStrategy")}</label>
        <select
          value={strategy}
          onChange={(e) => setStrategy(e.target.value as RegressionPlanStrategy)}
          className="w-full theme-input border px-2 py-1 text-xs rounded mb-2"
        >
          <option value="category">category</option>
          <option value="feature">feature</option>
          <option value="relation">relation</option>
        </select>

        <label className="text-xs text-[var(--text-muted)]">{t("regressionCategories")}</label>
        <div className="flex flex-wrap gap-1 mb-2">
          {CATEGORY_OPTIONS.map((c) => {
            const active = categories.includes(c);
            return (
              <button
                key={c}
                type="button"
                onClick={() =>
                  setCategories((prev) =>
                    active ? prev.filter((x) => x !== c) : [...prev, c]
                  )
                }
                className={`px-2 py-0.5 text-[10px] rounded border ${
                  active ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary"
                }`}
              >
                {c}
              </button>
            );
          })}
        </div>

        <label className="text-xs text-[var(--text-muted)]">{t("regressionFeatureIds")}</label>
        <textarea
          value={featureIdsRaw}
          onChange={(e) => setFeatureIdsRaw(e.target.value)}
          rows={3}
          className="w-full theme-input border px-2 py-1 text-xs rounded mb-2"
        />

        <label className="text-xs text-[var(--text-muted)]">{t("regressionChangedFiles")}</label>
        <textarea
          value={changedFilesRaw}
          onChange={(e) => setChangedFilesRaw(e.target.value)}
          rows={4}
          className="w-full theme-input border px-2 py-1 text-xs rounded mb-2"
        />

        <div className="flex items-center gap-2 mb-2">
          <input
            id="reg-indirect"
            type="checkbox"
            checked={includeIndirect}
            onChange={(e) => setIncludeIndirect(e.target.checked)}
          />
          <label htmlFor="reg-indirect" className="text-xs text-[var(--text-muted)]">
            {t("regressionIncludeIndirect")}
          </label>
        </div>

        <div className="flex items-center gap-2 mb-2">
          <input
            id="reg-msl-examples"
            type="checkbox"
            checked={includeModelicaExamples}
            onChange={(e) => setIncludeModelicaExamples(e.target.checked)}
          />
          <label htmlFor="reg-msl-examples" className="text-xs text-[var(--text-muted)]">
            {t("regressionIncludeModelicaExamples")}
          </label>
        </div>

        <div className="flex items-center gap-2 mb-2">
          <input
            id="reg-modelica-test"
            type="checkbox"
            checked={includeModelicaTest}
            onChange={(e) => setIncludeModelicaTest(e.target.checked)}
          />
          <label htmlFor="reg-modelica-test" className="text-xs text-[var(--text-muted)]">
            {t("regressionIncludeModelicaTest")}
          </label>
        </div>

        <label className="text-xs text-[var(--text-muted)]">{t("regressionMaxCases")}</label>
        <input
          value={maxCasesRaw}
          onChange={(e) => setMaxCasesRaw(e.target.value)}
          className="w-full theme-input border px-2 py-1 text-xs rounded mb-2"
          placeholder={t("regressionMaxCasesPlaceholder")}
        />

        <label className="text-xs text-[var(--text-muted)]">{t("regressionWorkspaceMode")}</label>
        <select
          value={workspaceMode}
          onChange={(e) => setWorkspaceMode(e.target.value as "persistent" | "ephemeral")}
          className="w-full theme-input border px-2 py-1 text-xs rounded mb-3"
        >
          <option value="persistent">{t("regressionPersistent")}</option>
          <option value="ephemeral">{t("regressionEphemeral")}</option>
        </select>

        <button
          type="button"
          onClick={createPlan}
          disabled={loading}
          className="w-full px-3 py-1.5 text-xs rounded bg-primary hover:bg-blue-600 disabled:opacity-50 mb-2"
        >
          {loading ? t("regressionCreating") : t("regressionCreatePlan")}
        </button>

        <div className="border-t border-border mt-3 pt-3 flex items-center justify-between gap-2">
          <label className="flex items-center gap-2 text-xs text-[var(--text-muted)]">
            <input type="checkbox" checked={autoRefresh} onChange={(e) => setAutoRefresh(e.target.checked)} />
            {t("refresh")} (3s)
          </label>
          <button
            type="button"
            onClick={() => refreshList().catch((e) => setMessage(String(e)))}
            className="px-2 py-1 text-xs rounded border theme-button-secondary"
          >
            {t("regressionRuns")}
          </button>
        </div>

        {message && (
          <div className="mt-3 text-xs break-all px-2 py-1.5 rounded border border-border bg-[var(--surface-muted)] text-[var(--text-muted)]">{message}</div>
        )}
      </aside>

      <main className="col-span-8 min-w-0 min-h-0 overflow-auto p-3">
        {!state ? (
          <div className="text-sm text-[var(--text-muted)]">{t("regressionNoWorkspaceSelected")}</div>
        ) : (
          <>
            <div className="grid grid-cols-4 gap-2 mb-3">
              <div className="border border-border rounded p-2">
                <div className="text-[10px] text-[var(--text-muted)]">{t("regressionWorkspace")}</div>
                <div className="text-xs font-mono">{state.info.workspaceId}</div>
              </div>
              <div className="border border-border rounded p-2">
                <div className="text-[10px] text-[var(--text-muted)]">{t("status")}</div>
                <div className={`inline-flex px-1.5 py-0.5 rounded text-xs border ${statusTone(state.info.status)}`}>{state.info.status}</div>
              </div>
              <div className="border border-border rounded p-2">
                <div className="text-[10px] text-[var(--text-muted)]">{t("regressionPlanCases")}</div>
                <div className="text-xs">{state.plan.plannedCases.length}</div>
              </div>
              <div className="border border-border rounded p-2">
                <div className="text-[10px] text-[var(--text-muted)]">{t("regressionSkipped")}</div>
                <div className="text-xs">{state.plan.skippedCases.length}</div>
              </div>
            </div>

            <div className="panel-header-min-height border border-border rounded bg-[var(--surface-elevated)] px-2 flex items-center justify-between mb-3">
              <div className="flex gap-1">
                <button
                  type="button"
                  onClick={() => setActiveTab("monitor")}
                  className={`px-2.5 py-1 text-xs rounded border ${activeTab === "monitor" ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary"}`}
                >
                  {t("regressionMonitor")}
                </button>
                <button
                  type="button"
                  onClick={() => setActiveTab("stats")}
                  className={`px-2.5 py-1 text-xs rounded border ${activeTab === "stats" ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary"}`}
                >
                  {t("regressionStatistics")}
                </button>
                <button
                  type="button"
                  onClick={() => setActiveTab("records")}
                  className={`px-2.5 py-1 text-xs rounded border ${activeTab === "records" ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary"}`}
                >
                  {t("regressionLatestRecords")}
                </button>
              </div>
              {activeTab === "records" && (
                <label className="flex items-center gap-2 text-xs text-[var(--text-muted)]">
                  <input type="checkbox" checked={showFailedOnly} onChange={(e) => setShowFailedOnly(e.target.checked)} />
                  {t("testFailed")}
                </label>
              )}
            </div>

            {activeTab === "monitor" && (
            <div className="grid grid-cols-2 gap-3">
              <div className="border border-border rounded p-2">
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionPlanCases")}</div>
                <div className="max-h-[420px] overflow-auto">
                  {state.plan.plannedCases.length === 0 ? (
                    <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
                  ) : (
                    state.plan.plannedCases.slice(0, 500).map((x) => (
                      <div key={x.name} className="text-xs py-1 border-b border-border/30 last:border-b-0">
                        <div className="font-mono truncate">{x.name}</div>
                        <div className="text-[10px] text-[var(--text-muted)]">{x.reason} | p{String(x.priority)}</div>
                      </div>
                    ))
                  )}
                </div>
              </div>
              <div className="border border-border rounded p-2">
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionSkipped")}</div>
                <div className="max-h-[420px] overflow-auto">
                  {state.plan.skippedCases.length === 0 ? (
                    <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
                  ) : (
                    state.plan.skippedCases.slice(0, 500).map((x) => (
                      <div key={x} className="text-xs py-1 border-b border-border/30 last:border-b-0 font-mono truncate">{x}</div>
                    ))
                  )}
                </div>
              </div>
            </div>
            )}

            {activeTab === "stats" && (
            <>
            <div className="grid grid-cols-4 gap-2 mb-3">
              <div className="border border-border rounded p-2">
                <div className="text-[10px] text-[var(--text-muted)]">{t("jitSummaryTotal")}</div>
                <div className="text-sm">{summary.total}</div>
              </div>
              <div className="border border-border rounded p-2">
                <div className="text-[10px] text-[var(--text-muted)]">{t("testPassed")}</div>
                <div className="text-sm text-[var(--success-text)]">{summary.passed}</div>
              </div>
              <div className="border border-border rounded p-2">
                <div className="text-[10px] text-[var(--text-muted)]">{t("testFailed")}</div>
                <div className="text-sm text-[var(--danger-text)]">{summary.failed}</div>
              </div>
              <div className="border border-border rounded p-2">
                <div className="text-[10px] text-[var(--text-muted)]">{t("successRate")}</div>
                <div className="text-sm">{pct(summary.passed, summary.total)}</div>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="border border-border rounded p-2">
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionFailByReason")}</div>
                <div className="max-h-48 overflow-auto">
                  {summary.failByReason.length === 0 ? (
                    <div className="text-xs text-[var(--text-muted)]">{t("regressionNoFailedRecord")}</div>
                  ) : (
                    summary.failByReason.map((x) => (
                      <div key={x.key} className="text-xs flex justify-between py-0.5">
                        <span className="truncate pr-2">{x.key}</span>
                        <span>{x.count}</span>
                      </div>
                    ))
                  )}
                </div>
              </div>
              <div className="border border-border rounded p-2">
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionPassByCategory")}</div>
                <div className="max-h-48 overflow-auto">
                  {summary.passByCategory.length === 0 ? (
                    <div className="text-xs text-[var(--text-muted)]">{t("regressionNoPassedRecord")}</div>
                  ) : (
                    summary.passByCategory.map((x) => (
                      <div key={x.key} className="text-xs flex justify-between py-0.5">
                        <span className="truncate pr-2">{x.key}</span>
                        <span>{x.count}</span>
                      </div>
                    ))
                  )}
                </div>
              </div>
            </div>
            </>
            )}

            {activeTab === "records" && (
            <div className="border border-border rounded p-2">
              <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionLatestRecords")}</div>
              <div className="max-h-[520px] overflow-auto">
                {visibleRecords.length === 0 ? (
                  <div className="text-xs text-[var(--text-muted)]">{t("regressionNoRecordsYet")}</div>
                ) : (
                  <>
                  <div className="sticky top-0 z-10 grid grid-cols-12 gap-2 px-2 py-1 text-[10px] uppercase text-[var(--text-muted)] bg-[var(--surface-elevated)] border-b border-border">
                    <div className="col-span-6">{t("name")}</div>
                    <div className="col-span-2">{t("status")}</div>
                    <div className="col-span-2">{t("exitLabel")}</div>
                    <div className="col-span-2">{t("durationLabel")}</div>
                  </div>
                  {visibleRecords.slice(0, 2000).map((r) => (
                    <div key={`${r.timestamp}-${r.caseName}`} className="grid grid-cols-12 gap-2 px-2 py-1 text-xs border-b border-border/30 last:border-b-0">
                      <div className="col-span-6">
                        <div className="font-mono truncate" title={r.caseName}>{r.caseName}</div>
                        <div className="text-[10px] text-[var(--text-muted)] truncate" title={r.reason}>{r.reason}</div>
                      </div>
                      <div className="col-span-2">
                        <span className={r.actualOk ? "text-[var(--success-text)]" : "text-[var(--danger-text)]"}>
                          {r.actualOk ? "OK" : "FAIL"}
                        </span>
                      </div>
                      <div className="col-span-2 text-[var(--text-muted)]">{String(r.exitCode)}</div>
                      <div className="col-span-2 text-[var(--text-muted)]">{r.durationMs}ms</div>
                    </div>
                  ))
                  }
                  </>
                )}
              </div>
            </div>
            )}
          </>
        )}
      </main>
      </div>
    </div>
  );
}

export default RegressionWorkspacePanel;
