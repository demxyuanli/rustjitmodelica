import { useState, useCallback, useEffect, useRef } from "react";
import { renderMarkdownToHtml, formatTimestamp } from "./ai-markdown";
import type { ChatMessage } from "./ai-markdown";
import { t } from "../../i18n";
import { AppIcon } from "../Icon";

interface AIChatMessageProps {
  message: ChatMessage;
  isLastAssistant: boolean;
  onCopy?: (text: string) => void;
  onRegenerate?: () => void;
  children?: React.ReactNode;
}

export function AIChatMessage({
  message,
  isLastAssistant,
  onCopy,
  onRegenerate,
  children,
}: AIChatMessageProps) {
  const isAssistant = message.role === "assistant";
  const [copied, setCopied] = useState(false);
  const msgRef = useRef<HTMLDivElement>(null);

  const handleCopy = useCallback(() => {
    const text = message.text;
    if (onCopy) {
      onCopy(text);
    } else {
      navigator.clipboard.writeText(text);
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }, [message.text, onCopy]);

  useEffect(() => {
    if (!msgRef.current) return;
    const el = msgRef.current;
    const copyBtns = el.querySelectorAll<HTMLButtonElement>(".ai-code-block-copy[data-copy-target]");
    const handler = (e: Event) => {
      const btn = e.currentTarget as HTMLButtonElement;
      const targetId = btn.getAttribute("data-copy-target");
      if (!targetId) return;
      const codeEl = document.getElementById(targetId);
      if (!codeEl) return;
      navigator.clipboard.writeText(codeEl.textContent ?? "").then(() => {
        btn.classList.add("ai-code-block-copy-done");
        setTimeout(() => btn.classList.remove("ai-code-block-copy-done"), 1500);
      });
    };
    copyBtns.forEach((btn) => btn.addEventListener("click", handler));
    return () => copyBtns.forEach((btn) => btn.removeEventListener("click", handler));
  }, [message.text]);

  return (
    <div
      ref={msgRef}
      className={`agent-msg agent-msg-enter ${isAssistant ? "agent-msg-assistant" : "agent-msg-user"}`}
    >
      <div className="agent-msg-header">
        <div className="agent-msg-avatar">
          {isAssistant ? (
            <AppIcon name="ai" className="w-3.5 h-3.5" />
          ) : (
            <AppIcon name="user" className="w-3.5 h-3.5" />
          )}
        </div>
        <span className="agent-msg-role">
          {isAssistant ? t("roleAssistant") : t("roleUser")}
        </span>
        <span className="agent-msg-time">{formatTimestamp(message.id)}</span>
      </div>

      {isAssistant ? (
        <div
          className="ai-md text-xs break-words"
          dangerouslySetInnerHTML={{ __html: renderMarkdownToHtml(message.text) }}
        />
      ) : (
        <div className="text-xs whitespace-pre-wrap break-words">{message.text}</div>
      )}

      {children}

      {isAssistant && (
        <div className={`agent-msg-actions ${isLastAssistant ? "agent-msg-actions-visible" : ""}`}>
          <button
            type="button"
            className="agent-msg-action-btn"
            onClick={handleCopy}
            title={copied ? t("copied") : t("copyMessage")}
          >
            {copied ? (
              <AppIcon name="check" className="w-3 h-3" />
            ) : (
              <AppIcon name="copy" className="w-3 h-3" />
            )}
          </button>
          {isLastAssistant && onRegenerate && (
            <button
              type="button"
              className="agent-msg-action-btn"
              onClick={onRegenerate}
              title={t("regenerate")}
            >
              <AppIcon name="refresh" className="w-3 h-3" />
            </button>
          )}
        </div>
      )}
    </div>
  );
}
