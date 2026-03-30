import type { Dispatch, SetStateAction } from "react";
import { t, tf } from "../../i18n";
import type { RegressionPlanStrategy, RegressionPlannedCase, RegressionWorkspaceState } from "../../types";
import { CATEGORY_OPTIONS } from "./regressionConstants";

export interface RegressionPlanSidebarProps {
  state: RegressionWorkspaceState | null;
  strategy: RegressionPlanStrategy;
  setStrategy: Dispatch<SetStateAction<RegressionPlanStrategy>>;
  categories: string[];
  setCategories: Dispatch<SetStateAction<string[]>>;
  featureIdsRaw: string;
  setFeatureIdsRaw: Dispatch<SetStateAction<string>>;
  changedFilesRaw: string;
  setChangedFilesRaw: Dispatch<SetStateAction<string>>;
  includeIndirect: boolean;
  setIncludeIndirect: Dispatch<SetStateAction<boolean>>;
  includeModelicaExamples: boolean;
  setIncludeModelicaExamples: Dispatch<SetStateAction<boolean>>;
  includeModelicaTest: boolean;
  setIncludeModelicaTest: Dispatch<SetStateAction<boolean>>;
  maxCasesRaw: string;
  setMaxCasesRaw: Dispatch<SetStateAction<string>>;
  workspaceMode: "persistent" | "ephemeral";
  setWorkspaceMode: Dispatch<SetStateAction<"persistent" | "ephemeral">>;
  onCreatePlan: () => void;
  loading: boolean;
  planCasesByCategory: RegressionPlannedCase[];
  planCaseQuery: string;
  setPlanCaseQuery: Dispatch<SetStateAction<string>>;
  autoRefresh: boolean;
  setAutoRefresh: Dispatch<SetStateAction<boolean>>;
  onRefreshList: () => Promise<void>;
  message: string | null;
  onError: (msg: string) => void;
}

export function RegressionPlanSidebar({
  state,
  strategy,
  setStrategy,
  categories,
  setCategories,
  featureIdsRaw,
  setFeatureIdsRaw,
  changedFilesRaw,
  setChangedFilesRaw,
  includeIndirect,
  setIncludeIndirect,
  includeModelicaExamples,
  setIncludeModelicaExamples,
  includeModelicaTest,
  setIncludeModelicaTest,
  maxCasesRaw,
  setMaxCasesRaw,
  workspaceMode,
  setWorkspaceMode,
  onCreatePlan,
  loading,
  planCasesByCategory,
  planCaseQuery,
  setPlanCaseQuery,
  autoRefresh,
  setAutoRefresh,
  onRefreshList,
  message,
  onError,
}: RegressionPlanSidebarProps) {
  return (
    <aside className="col-span-4 min-w-0 border-r border-border bg-[var(--panel-bg)] p-3 overflow-auto">
      <div className="text-xs uppercase text-[var(--text-muted)] mb-2">{t("regressionPlan")}</div>

      {state && (
        <div className="grid grid-cols-3 gap-1.5 mb-2">
          <div className="border border-border rounded p-1.5">
            <div className="text-[10px] text-[var(--text-muted)]">{t("regressionPlanCases")}</div>
            <div className="text-xs">{state.plan.plannedCases.length}</div>
          </div>
          <div className="border border-border rounded p-1.5">
            <div className="text-[10px] text-[var(--text-muted)]">{t("regressionChangedSources")}</div>
            <div className="text-xs">{state.plan.changedSources.length}</div>
          </div>
          <div className="border border-border rounded p-1.5">
            <div className="text-[10px] text-[var(--text-muted)]">{t("regressionAffectedFeatures")}</div>
            <div className="text-xs">{state.plan.affectedFeatures.length}</div>
          </div>
        </div>
      )}

      <details open className="border border-border rounded mb-2">
        <summary className="px-2 py-1.5 text-xs cursor-pointer select-none bg-[var(--surface-elevated)]">
          {t("regressionPlan")}
        </summary>
        <div className="p-2 space-y-2">
          <label className="text-xs text-[var(--text-muted)]">{t("regressionStrategy")}</label>
          <select
            value={strategy}
            onChange={(e) => setStrategy(e.target.value as RegressionPlanStrategy)}
            className="w-full theme-input border px-2 py-1 text-xs rounded"
          >
            <option value="category">category</option>
            <option value="feature">feature</option>
            <option value="relation">relation</option>
          </select>

          <label className="text-xs text-[var(--text-muted)]">{t("regressionCategories")}</label>
          <div className="flex flex-wrap gap-1">
            {CATEGORY_OPTIONS.map((c) => {
              const active = categories.includes(c);
              return (
                <button
                  key={c}
                  type="button"
                  onClick={() =>
                    setCategories((prev) => (active ? prev.filter((x) => x !== c) : [...prev, c]))
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

          <details className="border border-border/60 rounded">
            <summary className="px-2 py-1 text-[10px] cursor-pointer select-none text-[var(--text-muted)]">
              {t("regressionFeatureIds")} / {t("regressionChangedFiles")}
            </summary>
            <div className="p-2 space-y-2">
              <textarea
                value={featureIdsRaw}
                onChange={(e) => setFeatureIdsRaw(e.target.value)}
                rows={3}
                className="w-full theme-input border px-2 py-1 text-xs rounded"
              />
              <textarea
                value={changedFilesRaw}
                onChange={(e) => setChangedFilesRaw(e.target.value)}
                rows={4}
                className="w-full theme-input border px-2 py-1 text-xs rounded"
              />
            </div>
          </details>

          <div className="grid grid-cols-2 gap-2">
            <label className="flex items-center gap-1 text-[10px] text-[var(--text-muted)]">
              <input
                id="reg-indirect"
                type="checkbox"
                checked={includeIndirect}
                onChange={(e) => setIncludeIndirect(e.target.checked)}
              />
              {t("regressionIncludeIndirect")}
            </label>
            <label className="flex items-center gap-1 text-[10px] text-[var(--text-muted)]">
              <input
                id="reg-msl-examples"
                type="checkbox"
                checked={includeModelicaExamples}
                onChange={(e) => setIncludeModelicaExamples(e.target.checked)}
              />
              {t("regressionIncludeModelicaExamples")}
            </label>
            <label className="flex items-center gap-1 text-[10px] text-[var(--text-muted)] col-span-2">
              <input
                id="reg-modelica-test"
                type="checkbox"
                checked={includeModelicaTest}
                onChange={(e) => setIncludeModelicaTest(e.target.checked)}
              />
              {t("regressionIncludeModelicaTest")}
            </label>
          </div>

          <div className="grid grid-cols-2 gap-2">
            <div>
              <label className="text-xs text-[var(--text-muted)]">{t("regressionMaxCases")}</label>
              <input
                value={maxCasesRaw}
                onChange={(e) => setMaxCasesRaw(e.target.value)}
                className="w-full theme-input border px-2 py-1 text-xs rounded"
                placeholder={t("regressionMaxCasesPlaceholder")}
              />
            </div>
            <div>
              <label className="text-xs text-[var(--text-muted)]">{t("regressionWorkspaceMode")}</label>
              <select
                value={workspaceMode}
                onChange={(e) => setWorkspaceMode(e.target.value as "persistent" | "ephemeral")}
                className="w-full theme-input border px-2 py-1 text-xs rounded"
              >
                <option value="persistent">{t("regressionPersistent")}</option>
                <option value="ephemeral">{t("regressionEphemeral")}</option>
              </select>
            </div>
          </div>

          <button
            type="button"
            onClick={onCreatePlan}
            disabled={loading}
            className="w-full px-3 py-1.5 text-xs rounded bg-primary hover:bg-blue-600 disabled:opacity-50"
          >
            {loading ? t("regressionCreating") : t("regressionCreatePlan")}
          </button>
        </div>
      </details>

      {state && (
        <details open className="border border-border rounded mb-2">
          <summary className="px-2 py-1.5 text-xs cursor-pointer select-none bg-[var(--surface-elevated)]">
            {t("regressionPlanDetails")}
          </summary>
          <div className="p-2">
            <div className="text-[10px] text-[var(--text-muted)] mb-1">
              {tf("regressionPlanFilteredCount", {
                count: planCasesByCategory.length,
                total: state.plan.plannedCases.length,
              })}
            </div>
            <input
              value={planCaseQuery}
              onChange={(e) => setPlanCaseQuery(e.target.value)}
              placeholder={t("regressionFilterCaseName")}
              className="w-full theme-input border px-2 py-1 text-xs rounded mb-2"
            />
            <div className="max-h-44 overflow-auto border border-border/40 rounded bg-[var(--surface-muted)] p-1">
              {planCasesByCategory.length === 0 ? (
                <div className="text-xs text-[var(--text-muted)] px-1 py-1">{t("none")}</div>
              ) : (
                planCasesByCategory.slice(0, 300).map((x) => (
                  <div
                    key={`${x.name}-${x.category}`}
                    className="text-xs py-1 px-1 border-b border-border/20 last:border-b-0"
                  >
                    <div className="font-mono truncate" title={x.name}>
                      {x.name}
                    </div>
                    <div className="text-[10px] text-[var(--text-muted)] truncate">
                      {x.category} | {x.reason}
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        </details>
      )}

      {state && (
        <details className="border border-border rounded mb-2">
          <summary className="px-2 py-1.5 text-xs cursor-pointer select-none bg-[var(--surface-elevated)]">
            {t("regressionChangedSources")}
          </summary>
          <div className="p-2 max-h-28 overflow-auto">
            {state.plan.changedSources.length === 0 ? (
              <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
            ) : (
              state.plan.changedSources.slice(0, 200).map((s) => (
                <div key={s} className="text-[10px] font-mono truncate" title={s}>
                  {s}
                </div>
              ))
            )}
          </div>
        </details>
      )}

      {state && (
        <details className="border border-border rounded mb-2">
          <summary className="px-2 py-1.5 text-xs cursor-pointer select-none bg-[var(--surface-elevated)]">
            {t("regressionAffectedFeatures")}
          </summary>
          <div className="p-2 max-h-28 overflow-auto">
            {state.plan.affectedFeatures.length === 0 ? (
              <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
            ) : (
              state.plan.affectedFeatures.slice(0, 200).map((f) => (
                <div key={f} className="text-[10px] truncate" title={f}>
                  {f}
                </div>
              ))
            )}
          </div>
        </details>
      )}

      <div className="border-t border-border mt-3 pt-3 flex items-center justify-between gap-2">
        <label className="flex items-center gap-2 text-xs text-[var(--text-muted)]">
          <input type="checkbox" checked={autoRefresh} onChange={(e) => setAutoRefresh(e.target.checked)} />
          {t("refresh")} (3s)
        </label>
        <button
          type="button"
          onClick={() => onRefreshList().catch((e) => onError(String(e)))}
          className="px-2 py-1 text-xs rounded border theme-button-secondary"
        >
          {t("regressionRuns")}
        </button>
      </div>

      {message && (
        <div className="mt-3 text-xs break-all px-2 py-1.5 rounded border border-border bg-[var(--surface-muted)] text-[var(--text-muted)]">
          {message}
        </div>
      )}
    </aside>
  );
}
