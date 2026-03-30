import { t } from "../../i18n";
import type { RegressionPlannedCase } from "../../types";

export interface RegressionMonitorTabProps {
  planCasesByCategory: RegressionPlannedCase[];
  skippedCases: string[];
}

export function RegressionMonitorTab({ planCasesByCategory, skippedCases }: RegressionMonitorTabProps) {
  return (
    <div className="grid grid-cols-2 gap-3">
      <div className="border border-border rounded p-2">
        <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionPlanCases")}</div>
        <div className="max-h-[420px] overflow-auto">
          {planCasesByCategory.length === 0 ? (
            <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
          ) : (
            planCasesByCategory.slice(0, 500).map((x) => (
              <div key={x.name} className="text-xs py-1 border-b border-border/30 last:border-b-0">
                <div className="font-mono truncate">{x.name}</div>
                <div className="text-[10px] text-[var(--text-muted)]">
                  {x.reason} | p{String(x.priority)}
                </div>
              </div>
            ))
          )}
        </div>
      </div>
      <div className="border border-border rounded p-2">
        <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionSkipped")}</div>
        <div className="max-h-[420px] overflow-auto">
          {skippedCases.length === 0 ? (
            <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
          ) : (
            skippedCases.slice(0, 500).map((x) => (
              <div key={x} className="text-xs py-1 border-b border-border/30 last:border-b-0 font-mono truncate">
                {x}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
