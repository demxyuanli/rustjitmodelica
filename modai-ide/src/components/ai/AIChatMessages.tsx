import { useRef, useEffect, useState, useCallback } from "react";
import { parseDiff, Diff, Hunk, type FileData } from "react-diff-view";
import "react-diff-view/style/index.css";
import type { ChatMessage } from "./ai-markdown";
import {
  splitAnswerAndDiff,
  extractFirstCodeBlock,
  parseNewFileDiff,
  renderInlineDiff,
  suggestMoPathFromModelCode,
  isExistingFileDiff,
} from "./ai-markdown";
import { AIChatMessage } from "./AIChatMessage";
import { IterateDiffPreview } from "../IterateDiffPreview";
import { t } from "../../i18n";
import { AppIcon } from "../Icon";
import type { AgentMode } from "../../hooks/useAI";
import { ContextMenu } from "../ContextMenu";

type DiffViewType = "split" | "unified";

function AiDiffFileBlock({ file, viewType }: { file: FileData; viewType: DiffViewType }) {
  return (
    <Diff
      key={`${file.oldRevision}-${file.newRevision}`}
      viewType={viewType}
      diffType={file.type}
      hunks={file.hunks}
    >
      {(hunks) => hunks.map((hunk) => <Hunk key={hunk.content} hunk={hunk} />)}
    </Diff>
  );
}

interface AIChatMessagesProps {
  messages: ChatMessage[];
  aiLoading: boolean;
  agentMode: AgentMode;
  iterationDiff?: string | null;
  projectDir?: string | null;
  onCreateMoFile?: (relativePath: string, content: string) => Promise<void>;
  onRegenerate?: () => void;
  onApplyDiff?: (diff: string) => Promise<void>;
}

export function AIChatMessages({
  messages,
  aiLoading,
  agentMode,
  iterationDiff,
  projectDir,
  onCreateMoFile,
  onRegenerate,
  onApplyDiff,
}: AIChatMessagesProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [showScrollBtn, setShowScrollBtn] = useState(false);
  const [createFileLoading, setCreateFileLoading] = useState(false);
  const [createFileError, setCreateFileError] = useState<string | null>(null);
  const [diffViewType, setDiffViewType] = useState<DiffViewType>("split");
  const [applyDiffLoading, setApplyDiffLoading] = useState(false);
  const [applyDiffError, setApplyDiffError] = useState<string | null>(null);
  const [applyDiffSuccess, setApplyDiffSuccess] = useState(false);
  const [messageMenuVisible, setMessageMenuVisible] = useState(false);
  const [messageMenuPosition, setMessageMenuPosition] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const [messageMenuText, setMessageMenuText] = useState<string | null>(null);
  const [messageMenuRole, setMessageMenuRole] = useState<ChatMessage["role"] | null>(null);

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

  const existingFileDiff = diff && isExistingFileDiff(diff);
  const handleApplyDiff = useCallback(() => {
    if (!diff || !onApplyDiff) return;
    setApplyDiffError(null);
    setApplyDiffSuccess(false);
    setApplyDiffLoading(true);
    onApplyDiff(diff)
      .then(() => setApplyDiffSuccess(true))
      .catch((e) => setApplyDiffError(String(e)))
      .finally(() => setApplyDiffLoading(false));
  }, [diff, onApplyDiff]);

  let parsedDiffFiles: FileData[] = [];
  let diffParseError = false;
  if (existingFileDiff && diff) {
    try {
      parsedDiffFiles = parseDiff(diff, { nearbySequences: "zip" });
    } catch {
      diffParseError = true;
    }
  }

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
            <div
              key={m.id}
              onContextMenu={(event) => {
                event.preventDefault();
                setMessageMenuPosition({ x: event.clientX, y: event.clientY });
                setMessageMenuText(m.text ?? "");
                setMessageMenuRole(m.role);
                setMessageMenuVisible(true);
              }}
            >
              <AIChatMessage
                message={m}
                isLastAssistant={isLastAssistant}
                onRegenerate={isLastAssistant ? onRegenerate : undefined}
              >
              {isAssistant && isLastAssistant && (
                <>
                  {diff && agentMode !== "iterate" && (
                    <div className="agent-inline-diff-wrapper mt-2">
                      {existingFileDiff && !diffParseError && parsedDiffFiles.length > 0 ? (
                        <div className="rounded-lg border border-border bg-[var(--surface-elevated)] overflow-hidden">
                          <div className="shrink-0 flex items-center justify-between gap-2 px-2 py-1.5 border-b border-border flex-wrap">
                            <div className="flex items-center gap-1">
                              <button
                                type="button"
                                className={`text-xs px-1.5 py-0.5 rounded ${diffViewType === "split" ? "bg-[var(--surface-active)] text-[var(--text)]" : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)]"}`}
                                onClick={() => setDiffViewType("split")}
                              >
                                {t("diffSplit")}
                              </button>
                              <button
                                type="button"
                                className={`text-xs px-1.5 py-0.5 rounded ${diffViewType === "unified" ? "bg-[var(--surface-active)] text-[var(--text)]" : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)]"}`}
                                onClick={() => setDiffViewType("unified")}
                              >
                                {t("diffUnified")}
                              </button>
                            </div>
                            {projectDir && onApplyDiff && (
                              <button
                                type="button"
                                className="agent-action-btn agent-action-btn-primary text-xs"
                                disabled={applyDiffLoading}
                                onClick={handleApplyDiff}
                              >
                                {applyDiffLoading ? "..." : t("aiApplyToProject")}
                              </button>
                            )}
                          </div>
                          <div className="max-h-[280px] overflow-auto p-2 scroll-vscode">
                            <div className="diff" style={{ fontFamily: "var(--font-mono, Consolas, monospace)", fontSize: 12 }}>
                              {parsedDiffFiles.map((file, i) => (
                                <AiDiffFileBlock key={i} file={file} viewType={diffViewType} />
                              ))}
                            </div>
                          </div>
                          {applyDiffError && (
                            <div className="px-2 py-1.5 border-t border-border text-[10px] text-[var(--danger-text)]">
                              {applyDiffError}
                            </div>
                          )}
                          {applyDiffSuccess && (
                            <div className="px-2 py-1.5 border-t border-border text-[10px] text-[var(--success-text)]">
                              {t("aiApplyToProjectSuccess")}
                            </div>
                          )}
                        </div>
                      ) : (
                        renderInlineDiff(diff)
                      )}
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
            </div>
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
      <ContextMenu
        visible={messageMenuVisible}
        x={messageMenuPosition.x}
        y={messageMenuPosition.y}
        onClose={() => setMessageMenuVisible(false)}
        items={
          messageMenuText
            ? [
                {
                  id: "copy-message",
                  label: t("copyTestAllOutput"),
                  onClick: () => {
                    void navigator.clipboard.writeText(messageMenuText);
                  },
                },
                ...(messageMenuRole === "assistant"
                  ? (() => {
                      const block = extractFirstCodeBlock(messageMenuText);
                      return [
                        {
                          id: "copy-code",
                          label: t("contextCopyCode"),
                          disabled: !block?.content,
                          onClick: () => {
                            if (!block?.content) return;
                            void navigator.clipboard.writeText(block.content);
                          },
                        },
                      ];
                    })()
                  : []),
              ]
            : []
        }
      />
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
