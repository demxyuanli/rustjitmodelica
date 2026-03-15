import { t } from "../../i18n";
import { AppIcon } from "../Icon";
import type { AgentMode } from "../../hooks/useAI";

function getAgentModeLabel(mode: AgentMode): string {
  switch (mode) {
    case "explain":
      return t("aiModeExplain");
    case "edit-selection":
      return t("aiModeEditSelection");
    case "edit-file":
      return t("aiModeEditFile");
    case "generate":
      return t("aiModeGenerate");
    case "iterate":
      return t("aiModeIterate");
    default:
      return mode;
  }
}

interface AIChatHeaderProps {
  agentMode: AgentMode;
  hasMessages: boolean;
  onNewChat?: () => void;
}

export function AIChatHeader({ agentMode, hasMessages, onNewChat }: AIChatHeaderProps) {
  return (
    <div className="agent-header">
      <div className="agent-header-left">
        <AppIcon name="ai" className="w-4 h-4 text-[var(--accent)]" />
        <span className="agent-header-title">{t("aiAssistant")}</span>
      </div>
      <div className="agent-header-center">
        <span className="agent-header-mode-badge">
          {getAgentModeLabel(agentMode)}
        </span>
      </div>
      <div className="agent-header-right">
        {hasMessages && onNewChat && (
          <button
            type="button"
            className="agent-header-btn"
            onClick={onNewChat}
            title={t("newChat")}
          >
            <AppIcon name="newChat" className="w-3.5 h-3.5" />
          </button>
        )}
      </div>
    </div>
  );
}

export { getAgentModeLabel };
