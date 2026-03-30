import { t } from "../../i18n";
import type { RegressionWorkspaceInfo } from "../../types";

export interface RegressionWorkspaceHeaderProps {
  workspaces: RegressionWorkspaceInfo[];
  selectedWorkspaceId: string | null;
  onSelectWorkspace: (id: string | null) => void;
  onRun: () => void;
  onRefresh: () => void;
  onCancel: () => void;
  running: boolean;
}

export function RegressionWorkspaceHeader({
  workspaces,
  selectedWorkspaceId,
  onSelectWorkspace,
  onRun,
  onRefresh,
  onCancel,
  running,
}: RegressionWorkspaceHeaderProps) {
  return (
    <div className="panel-header-min-height shrink-0 border-b border-border bg-[var(--surface-elevated)] px-3 flex items-center justify-between gap-3">
      <div className="min-w-0">
        <div className="text-xs uppercase text-[var(--text-muted)]">{t("workspaceRegression")}</div>
        <div className="text-[11px] text-[var(--text-muted)] truncate">{t("testManagerDesc")}</div>
      </div>
      <div className="flex items-center gap-2 shrink-0">
        <select
          value={selectedWorkspaceId ?? ""}
          onChange={(e) => onSelectWorkspace(e.target.value || null)}
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
          onClick={onRun}
          disabled={!selectedWorkspaceId || running}
          className="px-2.5 py-1 text-xs rounded border theme-banner-success disabled:opacity-50"
        >
          {running ? t("running") : t("run")}
        </button>
        <button
          type="button"
          onClick={onRefresh}
          disabled={!selectedWorkspaceId}
          className="px-2.5 py-1 text-xs rounded border theme-button-secondary disabled:opacity-50"
        >
          {t("refresh")}
        </button>
        <button
          type="button"
          onClick={onCancel}
          disabled={!selectedWorkspaceId}
          className="px-2.5 py-1 text-xs rounded border theme-banner-danger disabled:opacity-50"
        >
          {t("cancel")}
        </button>
      </div>
    </div>
  );
}
