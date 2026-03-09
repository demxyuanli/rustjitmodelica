import { useState, useEffect } from "react";
import { t } from "../i18n";
import { JitOverview } from "./JitOverview";
import { FeatureCaseMap } from "./FeatureCaseMap";
import { SelfIterateUI } from "./SelfIterateUI";
import { SourceBrowser } from "./SourceBrowser";
import { TestManager } from "./TestManager";
import { TraceabilityView } from "./TraceabilityView";
import { AnalyticsDashboard } from "./AnalyticsDashboard";
import { loadTraceabilityConfig } from "../data/jit_regression_metadata";

type TabId = "source" | "tests" | "traceability" | "iterate" | "analytics" | "overview" | "map";

interface CompilerIterateWorkspaceProps {
  targetPrefill?: string | null;
  onClearPrefill?: () => void;
  repoRoot?: string | null;
}

const TABS: { id: TabId; labelKey: string }[] = [
  { id: "source", labelKey: "sourceBrowserTitle" },
  { id: "tests", labelKey: "testManagerTitle" },
  { id: "traceability", labelKey: "traceabilityTitle" },
  { id: "iterate", labelKey: "selfIterate" },
  { id: "analytics", labelKey: "analyticsTitle" },
  { id: "overview", labelKey: "jitOverviewTitle" },
  { id: "map", labelKey: "featureCaseMapTitle" },
];

export function CompilerIterateWorkspace({ targetPrefill, onClearPrefill, repoRoot }: CompilerIterateWorkspaceProps) {
  const [activeTab, setActiveTab] = useState<TabId>("source");

  useEffect(() => {
    loadTraceabilityConfig().catch(() => {});
  }, []);

  useEffect(() => {
    if (targetPrefill) {
      setActiveTab("iterate");
    }
  }, [targetPrefill]);

  return (
    <div className="flex-1 min-h-0 overflow-hidden flex flex-col bg-surface-alt">
      <div
        className="flex border-b border-gray-700 shrink-0 bg-[#2d2d2d] overflow-x-auto"
        role="tablist"
        aria-label="Self-iteration workspace tabs"
      >
        {TABS.map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={activeTab === tab.id}
            aria-controls={`panel-${tab.id}`}
            id={`tab-${tab.id}`}
            tabIndex={activeTab === tab.id ? 0 : -1}
            className={`px-4 py-2.5 text-sm font-medium border-b-2 transition-colors whitespace-nowrap ${
              activeTab === tab.id
                ? "border-primary text-[var(--text)]"
                : "border-transparent text-[var(--text-muted)] hover:text-[var(--text)]"
            }`}
            onClick={() => setActiveTab(tab.id)}
          >
            {t(tab.labelKey as Parameters<typeof t>[0])}
          </button>
        ))}
      </div>
      <div
        id={`panel-${activeTab}`}
        role="tabpanel"
        aria-labelledby={`tab-${activeTab}`}
        className="flex-1 min-h-0 overflow-hidden"
      >
        {activeTab === "source" && <SourceBrowser repoRoot={repoRoot} />}
        {activeTab === "tests" && <TestManager />}
        {activeTab === "traceability" && <TraceabilityView />}
        {activeTab === "iterate" && (
          <SelfIterateUI fullScreen targetPrefill={targetPrefill} onClearPrefill={onClearPrefill} repoRoot={repoRoot} />
        )}
        {activeTab === "analytics" && <AnalyticsDashboard />}
        {activeTab === "overview" && <JitOverview />}
        {activeTab === "map" && <FeatureCaseMap />}
      </div>
    </div>
  );
}
