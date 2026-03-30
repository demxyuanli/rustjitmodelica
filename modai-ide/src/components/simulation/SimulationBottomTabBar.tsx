import { t } from "../../i18n";
import { AppIcon } from "../Icon";
import { SimulationTabButton } from "./SimulationPanelChrome";
import type { BottomTab } from "./simulationBottomTabTypes";

export interface SimulationBottomTabBarProps {
  bottomTab: BottomTab;
  onBottomTabChange: (tab: BottomTab) => void;
  canShowDeps: boolean;
  problemsBadge: number;
}

export function SimulationBottomTabBar({
  bottomTab,
  onBottomTabChange,
  canShowDeps,
  problemsBadge,
}: SimulationBottomTabBarProps) {
  return (
    <div className="flex shrink-0 items-stretch border-b border-border bg-surface-alt">
      <SimulationTabButton
        active={bottomTab === "problems"}
        label={t("tabProblems")}
        icon={
          <AppIcon name="warning" className="!h-3.5 !w-3.5" aria-hidden="true" />
        }
        badge={problemsBadge > 0 ? problemsBadge : undefined}
        onClick={() => onBottomTabChange("problems")}
      />
      <SimulationTabButton
        active={bottomTab === "output"}
        label={t("tabOutput")}
        icon={
          <AppIcon name="output" className="!h-3.5 !w-3.5" aria-hidden="true" />
        }
        onClick={() => onBottomTabChange("output")}
      />
      <SimulationTabButton
        active={bottomTab === "results"}
        label={t("tabResults")}
        icon={
          <AppIcon name="chart" className="!h-3.5 !w-3.5" aria-hidden="true" />
        }
        onClick={() => onBottomTabChange("results")}
      />
      {canShowDeps && (
        <SimulationTabButton
          active={bottomTab === "deps"}
          label={t("tabDependencies")}
          icon={
            <AppIcon name="link" className="!h-3.5 !w-3.5" aria-hidden="true" />
          }
          onClick={() => onBottomTabChange("deps")}
        />
      )}
    </div>
  );
}
