import { t } from "../../i18n";

export type RegressionWorkspaceMainTab = "monitor" | "stats" | "records";

export interface RegressionWorkspaceTabChromeProps {
  activeTab: RegressionWorkspaceMainTab;
  onTabChange: (tab: RegressionWorkspaceMainTab) => void;
  showFailedOnly: boolean;
  onShowFailedOnlyChange: (value: boolean) => void;
  onFailedFirstView: () => void;
}

export function RegressionWorkspaceTabChrome({
  activeTab,
  onTabChange,
  showFailedOnly,
  onShowFailedOnlyChange,
  onFailedFirstView,
}: RegressionWorkspaceTabChromeProps) {
  return (
    <div className="panel-header-min-height border border-border rounded bg-[var(--surface-elevated)] px-2 flex items-center justify-between mb-3">
      <div className="flex gap-1">
        <button
          type="button"
          onClick={() => onTabChange("monitor")}
          className={`px-2.5 py-1 text-xs rounded border ${activeTab === "monitor" ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary"}`}
        >
          {t("regressionMonitor")}
        </button>
        <button
          type="button"
          onClick={() => onTabChange("stats")}
          className={`px-2.5 py-1 text-xs rounded border ${activeTab === "stats" ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary"}`}
        >
          {t("regressionStatistics")}
        </button>
        <button
          type="button"
          onClick={() => onTabChange("records")}
          className={`px-2.5 py-1 text-xs rounded border ${activeTab === "records" ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary"}`}
        >
          {t("regressionLatestRecords")}
        </button>
      </div>
      {activeTab === "records" && (
        <div className="flex items-center gap-2">
          <button type="button" onClick={onFailedFirstView} className="px-2 py-1 text-xs rounded border theme-banner-danger">
            {t("regressionFailedFirstView")}
          </button>
          <label className="flex items-center gap-2 text-xs text-[var(--text-muted)]">
            <input type="checkbox" checked={showFailedOnly} onChange={(e) => onShowFailedOnlyChange(e.target.checked)} />
            {t("testFailed")}
          </label>
        </div>
      )}
    </div>
  );
}
