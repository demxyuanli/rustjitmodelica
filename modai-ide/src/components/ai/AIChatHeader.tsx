import { useState, useRef, useEffect } from "react";
import { t } from "../../i18n";
import { AppIcon } from "../Icon";
import type { AgentMode } from "../../hooks/useAI";
import type { AISession } from "../../hooks/useAISessions";

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

function formatSessionTime(ts: number): string {
  const d = new Date(ts);
  const now = new Date();
  const sameDay = d.toDateString() === now.toDateString();
  if (sameDay) {
    return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  }
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
}

interface AIChatHeaderProps {
  agentMode: AgentMode;
  hasMessages: boolean;
  onNewChat?: () => void;
  sessions?: AISession[];
  onLoadSession?: (id: string) => void;
  onDeleteSession?: (id: string) => void;
  onOpenRulesAndSkills?: () => void;
}

export function AIChatHeader({
  agentMode,
  onNewChat,
  sessions = [],
  onLoadSession,
  onDeleteSession,
  onOpenRulesAndSkills,
  hasMessages: _hasMessages,
}: AIChatHeaderProps) {
  const [historyOpen, setHistoryOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!historyOpen) return;
    const onOutside = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setHistoryOpen(false);
      }
    };
    window.addEventListener("click", onOutside);
    return () => window.removeEventListener("click", onOutside);
  }, [historyOpen]);

  return (
    <div className="agent-header">
      <div className="agent-header-left">
        <div title={t("aiAssistant")} aria-label={t("aiAssistant")}>
          <AppIcon name="ai" className="w-4 h-4 text-[var(--accent)]" />
        </div>
      </div>
      <div className="agent-header-center">
        <span className="agent-header-mode-badge">
          {getAgentModeLabel(agentMode)}
        </span>
      </div>
      <div className="agent-header-right flex items-center gap-0.5">
        {onNewChat && (
          <button
            type="button"
            className="agent-header-btn"
            onClick={onNewChat}
            title={t("aiNewAgent")}
          >
            <AppIcon name="newChat" className="w-3.5 h-3.5" />
          </button>
        )}
        <div className="relative" ref={dropdownRef}>
          <button
            type="button"
            className="agent-header-btn"
            onClick={() => setHistoryOpen((v) => !v)}
            title={t("aiHistory")}
          >
            <AppIcon name="history" className="w-3.5 h-3.5" />
          </button>
          {historyOpen && (
            <div className="absolute right-0 top-full mt-1 z-50 min-w-[200px] max-h-[240px] overflow-auto rounded border border-border bg-[var(--surface-elevated)] shadow-lg py-1">
              <div className="px-2 py-1 text-[10px] uppercase text-[var(--text-muted)] border-b border-border">
                {t("aiHistory")}
              </div>
              {sessions.length === 0 ? (
                <div className="px-2 py-3 text-xs text-[var(--text-muted)]">
                  {t("aiHistoryEmpty")}
                </div>
              ) : (
                sessions.slice().reverse().map((s) => (
                  <div
                    key={s.id}
                    className="flex items-center gap-1 group px-2 py-1.5 hover:bg-[var(--surface-hover)] cursor-pointer"
                  >
                    <button
                      type="button"
                      className="flex-1 min-w-0 text-left text-xs truncate"
                      onClick={() => {
                        onLoadSession?.(s.id);
                        setHistoryOpen(false);
                      }}
                    >
                      <span className="block truncate">{s.title}</span>
                      <span className="text-[10px] text-[var(--text-muted)]">{formatSessionTime(s.createdAt)}</span>
                    </button>
                    {onDeleteSession && (
                      <button
                        type="button"
                        className="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-[var(--danger-text)]/20 text-[var(--text-muted)]"
                        onClick={(e) => {
                          e.stopPropagation();
                          onDeleteSession(s.id);
                        }}
                        title={t("delete")}
                      >
                        ×
                      </button>
                    )}
                  </div>
                ))
              )}
            </div>
          )}
        </div>
        {onOpenRulesAndSkills && (
          <button
            type="button"
            className="agent-header-btn"
            onClick={onOpenRulesAndSkills}
            title={t("aiRulesAndSkills")}
          >
            <AppIcon name="settings" className="w-3.5 h-3.5" />
          </button>
        )}
      </div>
    </div>
  );
}

export { getAgentModeLabel };
