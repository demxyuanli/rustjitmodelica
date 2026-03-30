import type { Dispatch, SetStateAction } from "react";
import { t, tf } from "../../i18n";
import { BATCH_FIELD_OPTIONS, type BatchField, type CliCaseMode, type RecordPreset } from "./regressionConstants";
import { presetLabel, reasonLabel, recordKeyOf, workspaceStatusLabel } from "./regressionFormat";
import type { NormalizedRegressionRecord } from "./regressionTypes";

export interface SelectedAgg {
  reason: Array<{ key: string; count: number }>;
  status: Array<{ key: string; count: number }>;
  category: Array<{ key: string; count: number }>;
}

export interface RegressionRecordsTabProps {
  reasonFilter: string;
  setReasonFilter: Dispatch<SetStateAction<string>>;
  reasonOptions: string[];
  statusFilter: string;
  setStatusFilter: Dispatch<SetStateAction<string>>;
  statusOptions: string[];
  categoryFilter: string;
  setCategoryFilter: Dispatch<SetStateAction<string>>;
  categoryOptions: string[];
  caseQuery: string;
  setCaseQuery: Dispatch<SetStateAction<string>>;
  sortBy: "time" | "duration" | "name" | "reason" | "category";
  setSortBy: Dispatch<SetStateAction<"time" | "duration" | "name" | "reason" | "category">>;
  sortDir: "asc" | "desc";
  setSortDir: Dispatch<SetStateAction<"asc" | "desc">>;
  showFailedOnly: boolean;
  setShowFailedOnly: Dispatch<SetStateAction<boolean>>;
  failedFirst: boolean;
  setFailedFirst: Dispatch<SetStateAction<boolean>>;
  visibleRecords: NormalizedRegressionRecord[];
  visibleSummary: { passed: number; failed: number; total: number };
  applyRecordPreset: (preset: RecordPreset) => void;
  activePreset: RecordPreset;
  recordLimit: number;
  setRecordLimit: Dispatch<SetStateAction<number>>;
  exportVisibleCsv: () => void;
  exportFailedCsv: () => void;
  pageSize: number;
  setPageSize: Dispatch<SetStateAction<number>>;
  pageIndex: number;
  setPageIndex: Dispatch<SetStateAction<number>>;
  totalPages: number;
  selectAllFiltered: () => void;
  visibleSlice: NormalizedRegressionRecord[];
  cappedRecords: NormalizedRegressionRecord[];
  setSelectedRecordKeys: Dispatch<SetStateAction<string[]>>;
  unselectFailed: () => void;
  unselectPassed: () => void;
  invertVisibleSelection: () => void;
  selectByReasonQuick: () => void;
  selectByStatusQuick: () => void;
  copySelectedCaseNames: () => Promise<void>;
  copySelectedFailReasons: () => Promise<void>;
  exportSelectedCsv: () => void;
  exportSelectedJson: () => void;
  exportSelectedBatchListTxt: () => void;
  exportSelectedBatchListJson: () => void;
  copyBatchCliArgs: () => Promise<void>;
  onUserError: (msg: string) => void;
  selectedRecordsCount: number;
  selectedAgg: SelectedAgg;
  selectedBatchFields: BatchField[];
  setSelectedBatchFields: Dispatch<SetStateAction<BatchField[]>>;
  selectedBatchPreview: string[];
  cliCommandPrefix: string;
  setCliCommandPrefix: Dispatch<SetStateAction<string>>;
  cliCaseMode: CliCaseMode;
  setCliCaseMode: Dispatch<SetStateAction<CliCaseMode>>;
  cliShardSizeRaw: string;
  setCliShardSizeRaw: Dispatch<SetStateAction<string>>;
  cliCommands: string[];
  selectedRecordKeys: string[];
  lastClickedIndex: number | null;
  setLastClickedIndex: Dispatch<SetStateAction<number | null>>;
  resetFilters: () => void;
}

export function RegressionRecordsTab(p: RegressionRecordsTabProps) {
  return (
    <div className="border border-border rounded p-2">
      <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionLatestRecords")}</div>
      <div className="grid grid-cols-12 gap-2 mb-2">
        <div className="col-span-3">
          <label className="text-[10px] text-[var(--text-muted)]">{t("regressionFilterReason")}</label>
          <select
            value={p.reasonFilter}
            onChange={(e) => p.setReasonFilter(e.target.value)}
            className="w-full theme-input border px-2 py-1 text-xs rounded"
          >
            <option value="all">{t("all")}</option>
            {p.reasonOptions.map((x) => (
              <option key={x} value={x}>
                {reasonLabel(x)}
              </option>
            ))}
          </select>
        </div>
        <div className="col-span-2">
          <label className="text-[10px] text-[var(--text-muted)]">{t("regressionFilterStatus")}</label>
          <select
            value={p.statusFilter}
            onChange={(e) => p.setStatusFilter(e.target.value)}
            className="w-full theme-input border px-2 py-1 text-xs rounded"
          >
            <option value="all">{t("all")}</option>
            {p.statusOptions.map((x) => (
              <option key={x} value={x}>
                {workspaceStatusLabel(x)}
              </option>
            ))}
          </select>
        </div>
        <div className="col-span-2">
          <label className="text-[10px] text-[var(--text-muted)]">{t("regressionFilterCategory")}</label>
          <select
            value={p.categoryFilter}
            onChange={(e) => p.setCategoryFilter(e.target.value)}
            className="w-full theme-input border px-2 py-1 text-xs rounded"
          >
            <option value="all">{t("all")}</option>
            {p.categoryOptions.map((x) => (
              <option key={x} value={x}>
                {x}
              </option>
            ))}
          </select>
        </div>
        <div className="col-span-2">
          <label className="text-[10px] text-[var(--text-muted)]">{t("regressionFilterCaseName")}</label>
          <input
            value={p.caseQuery}
            onChange={(e) => p.setCaseQuery(e.target.value)}
            placeholder={t("searchPlaceholder")}
            className="w-full theme-input border px-2 py-1 text-xs rounded"
          />
        </div>
        <div className="col-span-2">
          <label className="text-[10px] text-[var(--text-muted)]">{t("regressionSortBy")}</label>
          <select
            value={p.sortBy}
            onChange={(e) =>
              p.setSortBy(e.target.value as "time" | "duration" | "name" | "reason" | "category")
            }
            className="w-full theme-input border px-2 py-1 text-xs rounded"
          >
            <option value="time">{t("regressionSortTime")}</option>
            <option value="duration">{t("regressionSortDuration")}</option>
            <option value="name">{t("regressionSortName")}</option>
            <option value="reason">{t("regressionSortReason")}</option>
            <option value="category">{t("regressionSortCategory")}</option>
          </select>
        </div>
        <div className="col-span-1">
          <label className="text-[10px] text-[var(--text-muted)]">{t("regressionSortDirection")}</label>
          <button
            type="button"
            onClick={() => p.setSortDir((prev) => (prev === "asc" ? "desc" : "asc"))}
            className="w-full px-2 py-1 text-xs rounded border theme-button-secondary"
            title={p.sortDir}
          >
            {p.sortDir === "asc" ? t("ascending") : t("descending")}
          </button>
        </div>
        <div className="col-span-2">
          <label className="text-[10px] text-[var(--text-muted)]">{t("view")}</label>
          <button
            type="button"
            onClick={p.resetFilters}
            className="w-full px-2 py-1 text-xs rounded border theme-button-secondary"
          >
            {t("regressionResetFilters")}
          </button>
        </div>
      </div>
      <div className="mb-2 flex items-center justify-between text-[10px] text-[var(--text-muted)]">
        <div>{tf("matchCount", { count: p.visibleRecords.length, files: 1 })}</div>
        <div className="flex items-center gap-2">
          <span className="px-1.5 py-0.5 rounded border border-border">
            {tf("regressionVisiblePassed", { count: p.visibleSummary.passed })}
          </span>
          <span className="px-1.5 py-0.5 rounded border border-border">
            {tf("regressionVisibleFailed", { count: p.visibleSummary.failed })}
          </span>
          <label className="flex items-center gap-1.5">
            <input type="checkbox" checked={p.failedFirst} onChange={(e) => p.setFailedFirst(e.target.checked)} />
            {t("regressionFailedFirst")}
          </label>
        </div>
      </div>
      <div className="mb-2 flex items-center gap-2">
        {(["latest", "failed-triage", "slowest"] as RecordPreset[]).map((preset) => (
          <button
            key={preset}
            type="button"
            onClick={() => p.applyRecordPreset(preset)}
            className={`px-2 py-1 text-xs rounded border ${
              p.activePreset === preset ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary"
            }`}
          >
            {presetLabel(preset)}
          </button>
        ))}
        <div className="ml-auto flex items-center gap-2">
          <label className="text-[10px] text-[var(--text-muted)]">{t("regressionRecordLimit")}</label>
          <input
            value={String(p.recordLimit)}
            onChange={(e) => {
              const n = Number(e.target.value);
              if (!Number.isFinite(n)) return;
              p.setRecordLimit(Math.max(50, Math.min(5000, Math.floor(n))));
            }}
            className="w-20 theme-input border px-2 py-1 text-xs rounded"
          />
          <button
            type="button"
            onClick={p.exportVisibleCsv}
            className="px-2 py-1 text-xs rounded border theme-button-secondary"
          >
            {t("regressionExportVisibleCsv")}
          </button>
          <button
            type="button"
            onClick={p.exportFailedCsv}
            className="px-2 py-1 text-xs rounded border theme-button-secondary"
          >
            {t("regressionExportFailedCsv")}
          </button>
          <label className="text-[10px] text-[var(--text-muted)]">{t("regressionPageSize")}</label>
          <select
            value={String(p.pageSize)}
            onChange={(e) => {
              const n = Number(e.target.value);
              p.setPageSize(n);
              p.setPageIndex(0);
            }}
            className="theme-input border px-2 py-1 text-xs rounded"
          >
            <option value="50">50</option>
            <option value="100">100</option>
            <option value="200">200</option>
            <option value="500">500</option>
          </select>
        </div>
      </div>
      <div className="mb-2 flex items-center gap-2">
        <button
          type="button"
          onClick={p.selectAllFiltered}
          className="px-2 py-1 text-xs rounded border theme-button-secondary"
        >
          {t("regressionSelectAllFiltered")}
        </button>
        <button
          type="button"
          onClick={() =>
            p.setSelectedRecordKeys((prev) => {
              const set = new Set(prev);
              for (const r of p.visibleSlice) set.add(recordKeyOf(r));
              return Array.from(set);
            })
          }
          className="px-2 py-1 text-xs rounded border theme-button-secondary"
        >
          {t("regressionSelectAllVisible")}
        </button>
        <button
          type="button"
          onClick={() => p.setSelectedRecordKeys([])}
          className="px-2 py-1 text-xs rounded border theme-button-secondary"
        >
          {t("regressionClearSelection")}
        </button>
        <button type="button" onClick={p.unselectFailed} className="px-2 py-1 text-xs rounded border theme-button-secondary">
          {t("regressionUnselectFailed")}
        </button>
        <button type="button" onClick={p.unselectPassed} className="px-2 py-1 text-xs rounded border theme-button-secondary">
          {t("regressionUnselectPassed")}
        </button>
        <button
          type="button"
          onClick={p.invertVisibleSelection}
          className="px-2 py-1 text-xs rounded border theme-button-secondary"
        >
          {t("regressionInvertVisibleSelection")}
        </button>
        <button
          type="button"
          onClick={p.selectByReasonQuick}
          className="px-2 py-1 text-xs rounded border theme-button-secondary"
        >
          {t("regressionSelectByReason")}
        </button>
        <button
          type="button"
          onClick={p.selectByStatusQuick}
          className="px-2 py-1 text-xs rounded border theme-button-secondary"
        >
          {t("regressionSelectByStatus")}
        </button>
        <button
          type="button"
          onClick={() => p.copySelectedCaseNames().catch((e) => p.onUserError(String(e)))}
          className="px-2 py-1 text-xs rounded border theme-button-secondary"
        >
          {t("regressionCopyCaseNames")}
        </button>
        <button
          type="button"
          onClick={() => p.copySelectedFailReasons().catch((e) => p.onUserError(String(e)))}
          className="px-2 py-1 text-xs rounded border theme-button-secondary"
        >
          {t("regressionCopyFailReasons")}
        </button>
        <button type="button" onClick={p.exportSelectedCsv} className="px-2 py-1 text-xs rounded border theme-button-secondary">
          {t("regressionExportSelectedCsv")}
        </button>
        <button type="button" onClick={p.exportSelectedJson} className="px-2 py-1 text-xs rounded border theme-button-secondary">
          {t("regressionExportSelectedJson")}
        </button>
        <button
          type="button"
          onClick={p.exportSelectedBatchListTxt}
          className="px-2 py-1 text-xs rounded border theme-button-secondary"
        >
          {t("regressionGenerateBatchListTxt")}
        </button>
        <button
          type="button"
          onClick={p.exportSelectedBatchListJson}
          className="px-2 py-1 text-xs rounded border theme-button-secondary"
        >
          {t("regressionGenerateBatchListJson")}
        </button>
        <button
          type="button"
          onClick={() => p.copyBatchCliArgs().catch((e) => p.onUserError(String(e)))}
          className="px-2 py-1 text-xs rounded border theme-button-secondary"
        >
          {t("regressionCopyCliArgs")}
        </button>
        <span className="text-[10px] text-[var(--text-muted)]">
          {tf("regressionSelectedCount", { count: p.selectedRecordsCount })}
        </span>
        <span className="text-[10px] text-[var(--text-muted)] ml-auto">
          {tf("regressionPageIndicator", { page: p.pageIndex + 1, total: p.totalPages })}
        </span>
        <button
          type="button"
          disabled={p.pageIndex <= 0}
          onClick={() => p.setPageIndex((x) => Math.max(0, x - 1))}
          className="px-2 py-1 text-xs rounded border theme-button-secondary disabled:opacity-50"
        >
          {t("prevCases")}
        </button>
        <button
          type="button"
          disabled={p.pageIndex >= p.totalPages - 1}
          onClick={() => p.setPageIndex((x) => Math.min(p.totalPages - 1, x + 1))}
          className="px-2 py-1 text-xs rounded border theme-button-secondary disabled:opacity-50"
        >
          {t("nextCases")}
        </button>
      </div>
      <div className="mb-2 grid grid-cols-3 gap-2">
        <div className="border border-border rounded p-2">
          <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionSelectedByReason")}</div>
          <div className="max-h-24 overflow-auto">
            {p.selectedAgg.reason.length === 0 ? (
              <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
            ) : (
              p.selectedAgg.reason.map((x) => (
                <div key={x.key} className="text-xs flex justify-between py-0.5">
                  <span className="truncate pr-2">{reasonLabel(x.key)}</span>
                  <span>{x.count}</span>
                </div>
              ))
            )}
          </div>
        </div>
        <div className="border border-border rounded p-2">
          <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionSelectedByStatus")}</div>
          <div className="max-h-24 overflow-auto">
            {p.selectedAgg.status.length === 0 ? (
              <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
            ) : (
              p.selectedAgg.status.map((x) => (
                <div key={x.key} className="text-xs flex justify-between py-0.5">
                  <span className="truncate pr-2">{workspaceStatusLabel(x.key)}</span>
                  <span>{x.count}</span>
                </div>
              ))
            )}
          </div>
        </div>
        <div className="border border-border rounded p-2">
          <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionSelectedByCategory")}</div>
          <div className="max-h-24 overflow-auto">
            {p.selectedAgg.category.length === 0 ? (
              <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
            ) : (
              p.selectedAgg.category.map((x) => (
                <div key={x.key} className="text-xs flex justify-between py-0.5">
                  <span className="truncate pr-2">{x.key}</span>
                  <span>{x.count}</span>
                </div>
              ))
            )}
          </div>
        </div>
      </div>
      <div className="mb-2 border border-border rounded p-2">
        <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionBatchTemplateFields")}</div>
        <div className="flex flex-wrap gap-1 mb-2">
          {BATCH_FIELD_OPTIONS.map((opt) => {
            const active = p.selectedBatchFields.includes(opt.key);
            return (
              <button
                key={opt.key}
                type="button"
                onClick={() =>
                  p.setSelectedBatchFields((prev) => {
                    if (active) {
                      if (prev.length <= 1) return prev;
                      return prev.filter((x) => x !== opt.key);
                    }
                    return [...prev, opt.key];
                  })
                }
                className={`px-2 py-0.5 text-[10px] rounded border ${
                  active ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary"
                }`}
              >
                {t(opt.labelKey)}
              </button>
            );
          })}
        </div>
        <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionBatchPreview")}</div>
        <div className="max-h-24 overflow-auto border border-border/50 rounded bg-[var(--surface-muted)] p-2">
          {p.selectedBatchPreview.length === 0 ? (
            <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
          ) : (
            p.selectedBatchPreview.map((line, idx) => (
              <div key={`preview-${idx}`} className="text-[10px] font-mono truncate" title={line}>
                {line}
              </div>
            ))
          )}
        </div>
        <div className="mt-2 grid grid-cols-12 gap-2">
          <div className="col-span-6">
            <label className="text-[10px] text-[var(--text-muted)]">{t("regressionCliCommandPrefix")}</label>
            <input
              value={p.cliCommandPrefix}
              onChange={(e) => p.setCliCommandPrefix(e.target.value)}
              className="w-full theme-input border px-2 py-1 text-xs rounded"
              placeholder="modai-worker run-batch"
            />
          </div>
          <div className="col-span-3">
            <label className="text-[10px] text-[var(--text-muted)]">{t("regressionCliCaseMode")}</label>
            <select
              value={p.cliCaseMode}
              onChange={(e) => p.setCliCaseMode(e.target.value as CliCaseMode)}
              className="w-full theme-input border px-2 py-1 text-xs rounded"
            >
              <option value="combined">{t("regressionCliCaseModeCombined")}</option>
              <option value="repeated">{t("regressionCliCaseModeRepeated")}</option>
            </select>
          </div>
          <div className="col-span-3">
            <label className="text-[10px] text-[var(--text-muted)]">{t("regressionCliShardSize")}</label>
            <input
              value={p.cliShardSizeRaw}
              onChange={(e) => p.setCliShardSizeRaw(e.target.value)}
              className="w-full theme-input border px-2 py-1 text-xs rounded"
            />
          </div>
          <div className="col-span-12 text-[10px] text-[var(--text-muted)]">
            {tf("regressionCliShardCount", { count: p.cliCommands.length })}
          </div>
        </div>
        <div className="text-[10px] uppercase text-[var(--text-muted)] mt-2 mb-1">{t("regressionCliPreview")}</div>
        <div className="max-h-24 overflow-auto border border-border/50 rounded bg-[var(--surface-muted)] p-2">
          {p.cliCommands.length === 0 ? (
            <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
          ) : (
            p.cliCommands.map((line, idx) => (
              <div key={`cli-${idx}`} className="text-[10px] font-mono truncate" title={line}>
                {line}
              </div>
            ))
          )}
        </div>
      </div>
      <div className="max-h-[520px] overflow-auto">
        {p.visibleRecords.length === 0 ? (
          <div className="text-xs text-[var(--text-muted)]">{t("regressionNoRecordsYet")}</div>
        ) : (
          <>
            <div className="sticky top-0 z-10 grid grid-cols-12 gap-2 px-2 py-1 text-[10px] uppercase text-[var(--text-muted)] bg-[var(--surface-elevated)] border-b border-border">
              <div className="col-span-1">{t("select")}</div>
              <div className="col-span-4">{t("name")}</div>
              <div className="col-span-2">{t("regressionFilterCategory")}</div>
              <div className="col-span-2">{t("status")}</div>
              <div className="col-span-2">{t("exitLabel")}</div>
              <div className="col-span-1">{t("durationLabel")}</div>
            </div>
            {p.visibleSlice.map((r, localIdx) => (
              <div
                key={recordKeyOf(r)}
                className="grid grid-cols-12 gap-2 px-2 py-1 text-xs border-b border-border/30 last:border-b-0"
              >
                <div className="col-span-1">
                  <input
                    type="checkbox"
                    checked={p.selectedRecordKeys.includes(recordKeyOf(r))}
                    onChange={(e) => {
                      const key = recordKeyOf(r);
                      const absoluteIdx = p.pageIndex * p.pageSize + localIdx;
                      const shift = (window.event as MouseEvent | undefined)?.shiftKey ?? false;
                      p.setSelectedRecordKeys((prev) => {
                        if (!shift || p.lastClickedIndex === null) {
                          if (e.target.checked) {
                            if (prev.includes(key)) return prev;
                            return [...prev, key];
                          }
                          return prev.filter((x) => x !== key);
                        }
                        const [a, b] =
                          absoluteIdx >= p.lastClickedIndex
                            ? [p.lastClickedIndex, absoluteIdx]
                            : [absoluteIdx, p.lastClickedIndex];
                        const keysInRange = p.cappedRecords.slice(a, b + 1).map((x) => recordKeyOf(x));
                        const set = new Set(prev);
                        if (e.target.checked) {
                          for (const k of keysInRange) set.add(k);
                        } else {
                          for (const k of keysInRange) set.delete(k);
                        }
                        return Array.from(set);
                      });
                      p.setLastClickedIndex(absoluteIdx);
                    }}
                  />
                </div>
                <div className="col-span-4">
                  <div className="font-mono truncate" title={r.caseName}>
                    {r.caseName}
                  </div>
                  <div className="text-[10px] text-[var(--text-muted)] truncate" title={reasonLabel(r.reason)}>
                    {reasonLabel(r.reason)}
                  </div>
                </div>
                <div className="col-span-2 text-[var(--text-muted)] truncate" title={r.parsedCategory}>
                  {r.parsedCategory}
                </div>
                <div className="col-span-2">
                  <span className={r.actualOk ? "text-[var(--success-text)]" : "text-[var(--danger-text)]"}>
                    {r.actualOk ? t("testPassed") : t("testFailed")}
                  </span>
                </div>
                <div className="col-span-2 text-[var(--text-muted)]">{String(r.exitCode)}</div>
                <div className="col-span-1 text-[var(--text-muted)]">{r.durationMs}ms</div>
              </div>
            ))}
          </>
        )}
      </div>
    </div>
  );
}
