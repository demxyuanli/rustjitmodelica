import { t } from "../../i18n";
import { AppIcon } from "../Icon";
import type { AgentMode } from "../../hooks/useAI";

interface AIChatErrorBannerProps {
  lastJitErrorText: string;
  agentMode: AgentMode;
  setAgentMode?: (m: AgentMode) => void;
  setAiPrompt: (v: string) => void;
}

export function AIChatErrorBanner({
  lastJitErrorText,
  agentMode,
  setAgentMode,
  setAiPrompt,
}: AIChatErrorBannerProps) {
  if (!lastJitErrorText.trim() || agentMode === "iterate") return null;

  return (
    <div className="agent-error-banner">
      <div className="agent-error-banner-header">
        <AppIcon name="warning" className="w-3.5 h-3.5 text-[var(--warning-text)]" />
        <span className="font-medium">{t("compilationFailed")}</span>
      </div>
      <div className="agent-error-banner-body">{lastJitErrorText}</div>
      <div className="agent-error-banner-actions">
        <button
          type="button"
          className="agent-action-btn agent-action-btn-primary"
          onClick={() => {
            setAgentMode?.("edit-selection");
            setAiPrompt(`Fix this Modelica compile error:\n${lastJitErrorText}`);
          }}
        >
          {t("aiFixMyCode")}
        </button>
        <button
          type="button"
          className="agent-action-btn"
          onClick={() => {
            setAgentMode?.("iterate");
            setAiPrompt(`Extend the compiler to support this failing case:\n${lastJitErrorText}`);
          }}
        >
          {t("aiExtendCompiler")}
        </button>
      </div>
    </div>
  );
}
