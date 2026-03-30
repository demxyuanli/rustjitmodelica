import { t } from "../../i18n";
import { pct, reasonLabel } from "./regressionFormat";

export interface RegressionStatsSummary {
  total: number;
  passed: number;
  failed: number;
  failByReason: Array<{ key: string; count: number }>;
  passByCategory: Array<{ key: string; count: number }>;
}

export interface RegressionStatsTabProps {
  summary: RegressionStatsSummary;
  onOpenRecordsForReason: (reason: string) => void;
  onOpenRecordsForCategory: (category: string) => void;
}

export function RegressionStatsTab({
  summary,
  onOpenRecordsForReason,
  onOpenRecordsForCategory,
}: RegressionStatsTabProps) {
  return (
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
                <button
                  type="button"
                  key={x.key}
                  className="w-full text-xs flex justify-between py-0.5 hover:bg-[var(--surface-muted)] rounded"
                  onClick={() => onOpenRecordsForReason(x.key)}
                >
                  <span className="truncate pr-2">{reasonLabel(x.key)}</span>
                  <span>{x.count}</span>
                </button>
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
                <button
                  type="button"
                  key={x.key}
                  className="w-full text-xs flex justify-between py-0.5 hover:bg-[var(--surface-muted)] rounded"
                  onClick={() => onOpenRecordsForCategory(x.key)}
                >
                  <span className="truncate pr-2">{x.key}</span>
                  <span>{x.count}</span>
                </button>
              ))
            )}
          </div>
        </div>
      </div>
    </>
  );
}
