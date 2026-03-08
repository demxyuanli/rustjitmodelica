import { useState } from "react";
import { t } from "../i18n";
import { JitOverview } from "./JitOverview";
import { FeatureCaseMap } from "./FeatureCaseMap";
import { SelfIterateUI } from "./SelfIterateUI";

type TabId = "overview" | "map" | "iterate";

interface CompilerIterateWorkspaceProps {
  targetPrefill?: string | null;
  onClearPrefill?: () => void;
}

export function CompilerIterateWorkspace({ targetPrefill, onClearPrefill }: CompilerIterateWorkspaceProps) {
  const [activeTab, setActiveTab] = useState<TabId>("overview");

  return (
    <div className="flex-1 min-h-0 overflow-hidden flex flex-col bg-surface-alt">
      <div
        className="flex border-b border-gray-700 shrink-0 bg-[#2d2d2d] rounded-t-lg"
        role="tablist"
        aria-label="Self-iteration workspace tabs"
      >
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === "overview"}
          aria-controls="panel-overview"
          id="tab-overview"
          tabIndex={activeTab === "overview" ? 0 : -1}
          className={`px-4 py-2.5 text-sm font-medium border-b-2 transition-colors rounded-t-lg ${
            activeTab === "overview"
              ? "border-primary text-[var(--text)]"
              : "border-transparent text-[var(--text-muted)] hover:text-[var(--text)]"
          }`}
          onClick={() => setActiveTab("overview")}
        >
          {t("jitOverviewTitle")}
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === "map"}
          aria-controls="panel-map"
          id="tab-map"
          tabIndex={activeTab === "map" ? 0 : -1}
          className={`px-4 py-2.5 text-sm font-medium border-b-2 transition-colors rounded-t-lg ${
            activeTab === "map"
              ? "border-primary text-[var(--text)]"
              : "border-transparent text-[var(--text-muted)] hover:text-[var(--text)]"
          }`}
          onClick={() => setActiveTab("map")}
        >
          {t("featureCaseMapTitle")}
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === "iterate"}
          aria-controls="panel-iterate"
          id="tab-iterate"
          tabIndex={activeTab === "iterate" ? 0 : -1}
          className={`px-4 py-2.5 text-sm font-medium border-b-2 transition-colors rounded-t-lg ${
            activeTab === "iterate"
              ? "border-primary text-[var(--text)]"
              : "border-transparent text-[var(--text-muted)] hover:text-[var(--text)]"
          }`}
          onClick={() => setActiveTab("iterate")}
        >
          {t("selfIterate")}
        </button>
      </div>
      <div
        id={`panel-${activeTab}`}
        role="tabpanel"
        aria-labelledby={`tab-${activeTab}`}
        className="flex-1 min-h-0 overflow-hidden"
      >
        {activeTab === "overview" && <JitOverview />}
        {activeTab === "map" && <FeatureCaseMap />}
        {activeTab === "iterate" && (
          <SelfIterateUI fullScreen targetPrefill={targetPrefill} onClearPrefill={onClearPrefill} />
        )}
      </div>
    </div>
  );
}
