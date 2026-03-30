import { t } from "../../i18n";
import type { RegressionWorkspaceState } from "../../types";
import { statusTone, workspaceStatusLabel } from "./regressionFormat";

export function RegressionWorkspaceSummaryGrid({ state }: { state: RegressionWorkspaceState }) {
  return (
    <div className="grid grid-cols-4 gap-2 mb-3">
      <div className="border border-border rounded p-2">
        <div className="text-[10px] text-[var(--text-muted)]">{t("regressionWorkspace")}</div>
        <div className="text-xs font-mono">{state.info.workspaceId}</div>
      </div>
      <div className="border border-border rounded p-2">
        <div className="text-[10px] text-[var(--text-muted)]">{t("status")}</div>
        <div className={`inline-flex px-1.5 py-0.5 rounded text-xs border ${statusTone(state.info.status)}`}>
          {workspaceStatusLabel(state.info.status)}
        </div>
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
  );
}
