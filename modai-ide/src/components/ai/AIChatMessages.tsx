import { useRef, useEffect, useState, useCallback } from "react";
import type { ChatMessage } from "./ai-markdown";
import { splitAnswerAndDiff, extractFirstCodeBlock, parseNewFileDiff, renderInlineDiff, suggestMoPathFromModelCode } from "./ai-markdown";
import { AIChatMessage } from "./AIChatMessage";
import { IterateDiffPreview } from "../IterateDiffPreview";
import { t } from "../../i18n";
import { AppIcon } from "../Icon";
import type { AgentMode } from "../../hooks/useAI";

interface AIChatMessagesProps {
  messages: ChatMessage[];
  aiLoading: boolean;
  agentMode: AgentMode;
  iterationDiff?: string | null;
  projectDir?: string | null;
  onCreateMoFile?: (relativePath: string, content: string) => Promise<void>;
  onRegenerate?: () => void;
}

export function AIChatMessages({
  messages,
  aiLoading,
  agentMode,
  iterationDiff,
  projectDir,
  onCreateMoFile,
  onRegenerate,
}: AIChatMessagesProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [showScrollBtn, setShowScrollBtn] = useState(false);
  const [createFileLoading, setCreateFileLoading] = useState(false);
  const [createFileError, setCreateFileError] = useState<string | null>(null);

  const scrollToBottom = useCallback(() => {
    const el = scrollRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
    }
  }, []);

  useEffect(() => {
    scrollToBottom();
  }, [messages.length, aiLoading, scrollToBottom]);

  const handleScroll = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 60;
    setShowScrollBtn(!atBottom);
  }, []);

  const lastAnswerText = (() => {
    for (let i = messages.length - 1; i >= 0; i -= 1) {
      if (messages[i]?.role === "assistant") return messages[i]?.text ?? "";
    }
    return "";
  })();

  const { diff } = splitAnswerAndDiff(lastAnswerText);
  const codeBlock = extractFirstCodeBlock(lastAnswerText);
  const newFileFromDiff = diff ? parseNewFileDiff(diff) : null;

  const handleCreateMoFromCodeBlock = useCallback(() => {
    if (!codeBlock?.content || !projectDir || !onCreateMoFile) return;
    const suggested = suggestMoPathFromModelCode(codeBlock.content);
    const path = window.prompt(t("aiRelativePathPrompt"), suggested);
    if (!path?.trim()) return;
    const normalized = path.trim().replace(/\\/g, "/");
    if (!normalized.endsWith(".mo")) {
      setCreateFileError(t("aiPathMustEndWithMo"));
      return;
    }
    setCreateFileError(null);
    setCreateFileLoading(true);
    onCreateMoFile(normalized, codeBlock.content)
      .catch((e) => setCreateFileError(String(e)))
      .finally(() => setCreateFileLoading(false));
  }, [codeBlock, projectDir, onCreateMoFile]);

  const handleCreateFileFromDiff = useCallback(() => {
    if (!newFileFromDiff || !projectDir || !onCreateMoFile) return;
    setCreateFileError(null);
    setCreateFileLoading(true);
    onCreateMoFile(newFileFromDiff.path, newFileFromDiff.content)
      .catch((e) => setCreateFileError(String(e)))
      .finally(() => setCreateFileLoading(false));
  }, [newFileFromDiff, projectDir, onCreateMoFile]);

  return (
    <div className="agent-messages-container">
      <div
        ref={scrollRef}
        className="flex-1 min-h-0 overflow-auto scroll-vscode space-y-3 pr-1"
        onScroll={handleScroll}
      >
        {messages.map((m, idx) => {
          const isAssistant = m.role === "assistant";
          const isLastAssistant =
            isAssistant && messages.slice(idx + 1).every((next) => next.role !== "assistant");

          return (
            <AIChatMessage
              key={m.id}
              message={m}
              isLastAssistant={isLastAssistant}
              onRegenerate={isLastAssistant ? onRegenerate : undefined}
            >
              {isAssistant && isLastAssistant && (
                <>
                  {diff && agentMode !== "iterate" && (
                    <div className="agent-inline-diff-wrapper">
                      {renderInlineDiff(diff)}
                    </div>
                  )}
                  {iterationDiff && agentMode === "iterate" && (
                    <div className="mt-2">
                      <IterateDiffPreview diff={iterationDiff} defaultExpanded />
                    </div>
                  )}
                  {projectDir && onCreateMoFile && (
                    <div className="mt-2 flex flex-wrap items-center gap-2">
                      {newFileFromDiff && (
                        <button
                          type="button"
                          className="agent-action-btn agent-action-btn-primary"
                          disabled={createFileLoading}
                          onClick={handleCreateFileFromDiff}
                        >
                          {createFileLoading ? "..." : t("aiCreateFile")}
                        </button>
                      )}
                      {!newFileFromDiff && codeBlock?.content && (
                        <button
                          type="button"
                          className="agent-action-btn agent-action-btn-primary"
                          disabled={createFileLoading}
                          onClick={handleCreateMoFromCodeBlock}
                        >
                          {createFileLoading ? "..." : t("aiCreateMoFile")}
                        </button>
                      )}
                      {createFileError && (
                        <span className="text-[var(--danger-text)] text-[10px]">{createFileError}</span>
                      )}
                    </div>
                  )}
                </>
              )}
            </AIChatMessage>
          );
        })}
        {aiLoading && (
          <div className="agent-msg agent-msg-assistant agent-msg-loading-container">
            <div className="agent-msg-header">
              <div className="agent-msg-avatar">
                <AppIcon name="ai" className="w-3.5 h-3.5" />
              </div>
              <span className="agent-msg-role">{t("roleAssistant")}</span>
            </div>
            <div className="agent-thinking">
              <span className="agent-thinking-dot" />
              <span className="agent-thinking-dot" />
              <span className="agent-thinking-dot" />
            </div>
          </div>
        )}
      </div>
      {showScrollBtn && (
        <button
          type="button"
          className="agent-scroll-bottom"
          onClick={scrollToBottom}
          title={t("scrollToBottom")}
        >
          <AppIcon name="arrowDown" className="w-3.5 h-3.5" />
        </button>
      )}
    </div>
  );
}
