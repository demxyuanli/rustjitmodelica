import { t } from "../../i18n";
import { AppIcon } from "../Icon";
import type { AgentMode } from "../../hooks/useAI";

interface AIChatEmptyStateProps {
  setAgentMode?: (m: AgentMode) => void;
  setAiPrompt?: (v: string) => void;
}

const quickActions: Array<{ mode: AgentMode; icon: "search" | "diff" | "run" | "iterate"; labelKey: "aiModeExplain" | "aiModeEditSelection" | "aiModeGenerate" | "aiModeIterate" }> = [
  { mode: "explain", icon: "search", labelKey: "aiModeExplain" },
  { mode: "edit-selection", icon: "diff", labelKey: "aiModeEditSelection" },
  { mode: "generate", icon: "run", labelKey: "aiModeGenerate" },
  { mode: "iterate", icon: "iterate", labelKey: "aiModeIterate" },
];

export function AIChatEmptyState({ setAgentMode, setAiPrompt }: AIChatEmptyStateProps) {
  return (
    <div className="flex-1 min-h-0 flex flex-col items-center justify-center px-6 py-8">
      <div className="agent-empty-icon-wrapper">
        <AppIcon name="ai" className="w-8 h-8 text-[var(--accent)]" />
      </div>
      <div className="mt-4 text-sm font-semibold text-[var(--text)]">
        {t("aiAssistant")}
      </div>
      <div className="mt-1.5 text-xs text-[var(--text-muted)] text-center max-w-[260px] leading-relaxed">
        {t("aiAgentEmptyHint")}
      </div>
      <div className="mt-1 text-[10px] text-[var(--text-muted)] text-center max-w-[260px] opacity-90">
        {t("aiApplyDiffHint")}
      </div>
      {setAgentMode && (
        <div className="agent-empty-grid">
          {quickActions.map((action) => (
            <button
              key={action.mode}
              type="button"
              className="agent-empty-card"
              onClick={() => {
                setAgentMode(action.mode);
                setAiPrompt?.("");
              }}
            >
              <AppIcon name={action.icon} className="w-4 h-4 text-[var(--accent)] mb-1.5" />
              <span className="text-[11px] font-medium text-[var(--text)]">{t(action.labelKey)}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
