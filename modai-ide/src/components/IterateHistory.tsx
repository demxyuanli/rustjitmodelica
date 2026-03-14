import type { IterationRecord } from "../api/tauri";
import { t } from "../i18n";

interface IterateHistoryProps {
  history: IterationRecord[];
  onReuseDiff: (record: IterationRecord) => void;
  onViewDiff?: (record: IterationRecord) => void;
}

export function IterateHistory({
  history,
  onReuseDiff,
  onViewDiff,
}: IterateHistoryProps) {
  if (history.length === 0) {
    return (
      <div className="rounded-lg border border-border bg-[var(--surface-elevated)] px-4 py-6 text-xs text-[var(--text-muted)] text-center">
        {t("noHistoryYet")}
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-border bg-[var(--surface-elevated)] overflow-hidden">
      <div className="px-3 py-2 border-b border-border text-xs font-medium text-[var(--text)]">
        {t("iterationHistory")}
      </div>
      <div className="max-h-56 overflow-auto">
        {history.map((record) => (
          <div
            key={record.id}
            className="px-3 py-2 border-b border-border/60 last:border-b-0 flex items-start justify-between gap-3"
          >
            <div className="min-w-0">
              <div className="text-xs text-[var(--text)] truncate">
                #{record.id} {record.target || t("currentPatch")}
              </div>
              <div className="text-[10px] text-[var(--text-muted)] mt-1">
                {record.created_at?.slice(0, 19)} · {record.success ? t("pass") : t("fail")}
              </div>
            </div>
            <div className="shrink-0 flex gap-2">
              {record.diff && onViewDiff && (
                <button
                  type="button"
                  onClick={() => onViewDiff(record)}
                  className="px-2 py-1 text-[10px] rounded border theme-banner-info"
                >
                  {t("view")}
                </button>
              )}
              {record.diff && (
                <button
                  type="button"
                  onClick={() => onReuseDiff(record)}
                  className="px-2 py-1 text-[10px] rounded border theme-button-secondary text-[var(--text)]"
                >
                  {t("reuse")}
                </button>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
