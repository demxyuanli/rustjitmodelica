import { Boxes, Database, PanelBottom, PanelRight, Settings, SlidersHorizontal } from "lucide-react";
import type { ActivityId } from "../settings/studioSettings";
import { t } from "../i18n";

export function ActivityBar({
  activity,
  showSidebar,
  showPanel,
  showSecondary,
  onActivity,
  onTogglePanel,
  onToggleSecondary,
  onOpenSettings,
}: {
  activity: ActivityId;
  showSidebar: boolean;
  showPanel: boolean;
  showSecondary: boolean;
  onActivity: (id: ActivityId) => void;
  onTogglePanel: () => void;
  onToggleSecondary: () => void;
  onOpenSettings: () => void;
}) {
  return (
    <nav className="activity-bar" aria-label={t("activityBar")}>
      <button
        type="button"
        className={`activity-btn${showSidebar && activity === "explorer" ? " active" : ""}`}
        title={t("activityExplorer")}
        aria-label={t("activityExplorer")}
        onClick={() => onActivity("explorer")}
      >
        <Boxes size={20} strokeWidth={1.6} />
      </button>
      <button
        type="button"
        className={`activity-btn${showSidebar && activity === "properties" ? " active" : ""}`}
        title={t("activityProperties")}
        aria-label={t("activityProperties")}
        onClick={() => onActivity("properties")}
      >
        <SlidersHorizontal size={20} strokeWidth={1.6} />
      </button>
      <button
        type="button"
        className={`activity-btn${showSidebar && activity === "data" ? " active" : ""}`}
        title={t("activityData")}
        aria-label={t("activityData")}
        onClick={() => onActivity("data")}
      >
        <Database size={20} strokeWidth={1.6} />
      </button>
      <div className="activity-spacer" />
      <button
        type="button"
        className={`activity-btn${showSecondary ? " active" : ""}`}
        title={t("activitySecondary")}
        aria-label={t("activitySecondary")}
        onClick={onToggleSecondary}
      >
        <PanelRight size={18} strokeWidth={1.6} />
      </button>
      <button
        type="button"
        className={`activity-btn${showPanel ? " active" : ""}`}
        title={t("activityPanel")}
        aria-label={t("activityPanel")}
        onClick={onTogglePanel}
      >
        <PanelBottom size={18} strokeWidth={1.6} />
      </button>
      <button
        type="button"
        className="activity-btn"
        title={t("settings")}
        aria-label={t("settings")}
        onClick={onOpenSettings}
      >
        <Settings size={18} strokeWidth={1.6} />
      </button>
    </nav>
  );
}
