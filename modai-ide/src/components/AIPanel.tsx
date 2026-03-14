import React, { useMemo, useState, useCallback, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { indexRepoGetContext, indexGetContext } from "../api/tauri";
import type { IterationRecord, IterationRunResult } from "../api/tauri";
import { t, tf } from "../i18n";
import { AppIcon } from "./Icon";
import type { AgentMode, AiContextBlock, PendingPatch } from "../hooks/useAI";
import { IterateActions } from "./IterateActions";
import { IterateDiffPreview } from "./IterateDiffPreview";
import { IterateHistory } from "./IterateHistory";

interface ChunkInfo {
  id: number;
  fileId: number;
  lineStart: number;
  lineEnd: number;
  content: string;
  contextLabel: string | null;
  contentHash: string;
  filePath: string;
}

interface ChatMessage {
  id: number;
  role: "user" | "assistant";
  text: string;
}

function escapeHtml(s: string): string {
  return s
    .split("&").join("&amp;")
    .split("<").join("&lt;")
    .split(">").join("&gt;")
    .split('"').join("&quot;")
    .split("'").join("&#039;");
}

function renderMarkdownToHtml(md: string): string {
  const lines = md.split("\r\n").join("\n").split("\n");
  let html = "";
  let inCode = false;
  let codeLang = "";
  let inList = false;

  const closeList = () => {
    if (inList) {
      html += "</ul>";
      inList = false;
    }
  };

  for (const rawLine of lines) {
    const line = rawLine ?? "";
    const trimmed = line.trimEnd();

    if (trimmed.startsWith("```")) {
      if (!inCode) {
        closeList();
        inCode = true;
        codeLang = trimmed.slice(3).trim();
        html += `<pre class="ai-md-pre"><code class="ai-md-code" data-lang="${escapeHtml(codeLang)}">`;
      } else {
        inCode = false;
        codeLang = "";
        html += "</code></pre>";
      }
      continue;
    }

    if (inCode) {
      html += escapeHtml(line) + "\n";
      continue;
    }

    const headingMatch = trimmed.match(/^(#{1,3})\s+(.*)$/);
    if (headingMatch) {
      closeList();
      const level = headingMatch[1].length;
      const text = escapeHtml(headingMatch[2] ?? "");
      html += `<h${level} class="ai-md-h">${text}</h${level}>`;
      continue;
    }

    const listMatch = trimmed.match(/^-\s+(.*)$/);
    if (listMatch) {
      if (!inList) {
        html += '<ul class="ai-md-ul">';
        inList = true;
      }
      html += `<li class="ai-md-li">${escapeHtml(listMatch[1] ?? "")}</li>`;
      continue;
    }

    if (!trimmed) {
      closeList();
      html += '<div class="ai-md-spacer"></div>';
      continue;
    }

    closeList();
    const escaped = escapeHtml(trimmed)
      .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
      .replace(/`([^`]+?)`/g, '<code class="ai-md-inline">$1</code>');
    html += `<p class="ai-md-p">${escaped}</p>`;
  }

  closeList();
  if (inCode) {
    html += "</code></pre>";
  }
  return html;
}

function splitAnswerAndDiff(text: string | null): { answer: string; diff: string } {
  const src = (text ?? "").split("\r\n").join("\n");
  if (!src.trim()) return { answer: "", diff: "" };

  const fenced = src.match(/```diff\s*\n([\s\S]*?)\n```/i);
  if (fenced && fenced[1]) {
    const diff = fenced[1].trimEnd();
    const answer = src.replace(fenced[0], "").trim();
    return { answer, diff };
  }

  const idx = src.indexOf("diff --git ");
  if (idx >= 0) {
    const answer = src.slice(0, idx).trim();
    const diff = src.slice(idx).trimEnd();
    return { answer, diff };
  }

  return { answer: src.trim(), diff: "" };
}

function extractFirstCodeBlock(text: string | null): { lang: string; content: string } | null {
  if (!text?.trim()) return null;
  const match = text.match(/```(\w*)\s*\n([\s\S]*?)```/);
  if (!match) return null;
  return { lang: (match[1] ?? "").trim(), content: (match[2] ?? "").trimEnd() };
}

function suggestMoPathFromModelCode(code: string): string {
  const m = code.match(/\bmodel\s+(\w+)/);
  return m ? `TestLib/${m[1]}.mo` : "TestLib/NewModel.mo";
}

function parseNewFileDiff(diff: string): { path: string; content: string } | null {
  const lines = diff.split("\n");
  let path: string | null = null;
  let startCollect = false;
  const contentLines: string[] = [];
  for (const line of lines) {
    if (line.startsWith("+++ b/")) {
      if (path) break;
      path = line.slice(6).trim();
      startCollect = true;
      continue;
    }
    if (startCollect && line.startsWith("--- ")) break;
    if (startCollect && line.startsWith("+") && !line.startsWith("+++")) {
      contentLines.push(line.slice(1));
    }
  }
  if (!path || contentLines.length === 0) return null;
  return { path, content: contentLines.join("\n") };
}

function renderInlineDiff(diff: string): React.ReactElement | null {
  if (!diff.trim()) return null;
  const lines = diff.split("\n");
  return (
    <pre className="agent-diff-block">
      {lines.map((line, idx) => {
        let cls = "agent-diff-line";
        if (line.startsWith("+")) cls += " agent-diff-line-added";
        else if (line.startsWith("-")) cls += " agent-diff-line-removed";
        return (
          <div key={idx} className={cls}>
            {line}
          </div>
        );
      })}
    </pre>
  );
}

export interface AIPanelProps {
  apiKey: string;
  setApiKey: (v: string) => void;
  apiKeySaved: boolean;
  onSaveApiKey: (key: string) => void;
  aiPrompt: string;
  setAiPrompt: (v: string) => void;
  aiLoading: boolean;
  aiResponse: string | null;
  onSend: (extraContextBlocks?: AiContextBlock[]) => void;
  onInsert: () => void;
  tokenEstimate: number;
  dailyTokenUsed: number;
  dailyTokenLimit: number;
  sendDisabled: boolean;
  projectDir?: string | null;
  repoRoot?: string | null;
  mode: "chat" | "code";
  setMode: (m: "chat" | "code") => void;
  model: string;
  setModel: (m: string) => void;
  onCopyResult?: (v: string) => void;
  onOpenScratch?: (v: string) => void;
  currentFilePath?: string | null;
  currentSelectionText?: string | null;
  lastJitErrorText?: string | null;
  messages?: ChatMessage[];
  agentMode?: AgentMode;
  setAgentMode?: (m: AgentMode) => void;
  pendingPatch?: PendingPatch | null;
  clearPendingPatch?: () => void;
  onCreateMoFile?: (relativePath: string, content: string) => Promise<void>;
  iterationDiff?: string | null;
  iterationRunResult?: IterationRunResult | null;
  iterationHistory?: IterationRecord[];
  onRunIteration?: (quick: boolean) => Promise<unknown>;
  onAdoptIteration?: () => Promise<unknown>;
  onCommitIteration?: (message?: string) => Promise<unknown>;
  onReuseIteration?: (record: IterationRecord) => Promise<unknown>;
  theme?: "dark" | "light";
}

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

export function AIPanel({
  apiKeySaved: _apiKeySaved,
  aiPrompt,
  setAiPrompt,
  aiLoading,
  aiResponse,
  onSend,
  onInsert,
  tokenEstimate: _tokenEstimate,
  dailyTokenUsed: _dailyTokenUsed,
  dailyTokenLimit: _dailyTokenLimit,
  sendDisabled,
  projectDir,
  repoRoot,
  mode: _mode,
  setMode,
  model,
  setModel,
  currentFilePath,
  currentSelectionText,
  lastJitErrorText,
  messages: messagesProp,
  agentMode = "edit-selection",
  setAgentMode,
  pendingPatch,
  clearPendingPatch: _clearPendingPatch,
  onCreateMoFile,
  iterationDiff,
  iterationRunResult,
  iterationHistory = [],
  onRunIteration,
  onAdoptIteration,
  onCommitIteration,
  onReuseIteration,
  theme = "dark",
}: AIPanelProps) {
  const [contextChunks, setContextChunks] = useState<ChunkInfo[]>([]);
  const [contextLoading, setContextLoading] = useState(false);
  const [useContext, setUseContext] = useState(true);
  const [modeMenuOpen, setModeMenuOpen] = useState(false);
  const [localMessages, setLocalMessages] = useState<ChatMessage[]>([]);
  const lastAssistantRef = useRef<string | null>(null);
  const [fileBarOpen, setFileBarOpen] = useState(true);
  const inputRef = useRef<HTMLTextAreaElement | null>(null);
  const modeTriggerRef = useRef<HTMLButtonElement | null>(null);
  const [modeMenuRect, setModeMenuRect] = useState<{ bottom: number; left: number } | null>(null);

  const messages = messagesProp ?? localMessages;

  const fetchContext = useCallback(async (): Promise<ChunkInfo[]> => {
    if (!aiPrompt.trim()) {
      setContextChunks([]);
      return [];
    }
    setContextLoading(true);
    try {
      let chunks: ChunkInfo[] = [];
      if (repoRoot) {
        chunks = (await indexRepoGetContext(aiPrompt.trim(), 8)) as ChunkInfo[];
      } else if (projectDir) {
        chunks = (await indexGetContext(projectDir, aiPrompt.trim(), 8)) as ChunkInfo[];
      } else {
        setContextChunks([]);
        return [];
      }
      setContextChunks(chunks);
      return chunks;
    } catch {
      setContextChunks([]);
      return [];
    } finally {
      setContextLoading(false);
    }
  }, [projectDir, repoRoot, aiPrompt]);

  const handleSendWithContext = useCallback(() => {
    const promptText = aiPrompt.trim();
    if (!promptText) return;
    if (messagesProp === undefined) {
      setLocalMessages((prev) => [...prev, { id: Date.now(), role: "user", text: promptText }]);
    }
    if (useContext && (projectDir || repoRoot)) {
      fetchContext().then((chunks) => {
        const extra = chunks.map((c) => ({ path: c.filePath, content: c.content ?? "" }));
        onSend(extra.length > 0 ? extra : undefined);
      });
    } else {
      onSend();
    }
  }, [aiPrompt, useContext, projectDir, repoRoot, fetchContext, onSend, messagesProp]);

  const selectionPreview = useMemo(() => {
    const raw = (currentSelectionText ?? "").trim();
    if (!raw) return null;
    const max = 600;
    if (raw.length <= max) return raw;
    return raw.slice(0, max) + "\n... (truncated)";
  }, [currentSelectionText]);

  useEffect(() => {
    if (!aiResponse || messagesProp !== undefined) return;
    const text = aiResponse;
    if (!text.trim()) return;
    if (lastAssistantRef.current === text) return;
    lastAssistantRef.current = text;
    setLocalMessages((prev) => [...prev, { id: Date.now(), role: "assistant", text }]);
  }, [aiResponse, messagesProp]);

  const lastAnswerText = useMemo(() => {
    for (let i = messages.length - 1; i >= 0; i -= 1) {
      if (messages[i]?.role === "assistant") return messages[i]?.text ?? "";
    }
    return aiResponse ?? "";
  }, [messages, aiResponse]);

  const { diff } = useMemo(() => splitAnswerAndDiff(lastAnswerText), [lastAnswerText]);
  const codeBlock = useMemo(() => extractFirstCodeBlock(lastAnswerText), [lastAnswerText]);
  const newFileFromDiff = useMemo(() => (diff ? parseNewFileDiff(diff) : null), [diff]);
  const hasConversation = messages.length > 0;
  const [createFileLoading, setCreateFileLoading] = useState(false);
  const [createFileError, setCreateFileError] = useState<string | null>(null);
  const [iterationRunLoading, setIterationRunLoading] = useState(false);
  const [iterationAdoptLoading, setIterationAdoptLoading] = useState(false);
  const [iterationCommitLoading, setIterationCommitLoading] = useState(false);
  const [iterationCommitMessage, setIterationCommitMessage] = useState("");
  const canRunFull = !!iterationRunResult?.quick_run && !!iterationRunResult?.success;
  const canAdopt = !!iterationRunResult?.success && !!iterationDiff;
  const canCommit = !!iterationRunResult?.success && !iterationDiff;

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
  const hasPendingPatch = !!pendingPatch?.newContent;
  const fileCount = hasPendingPatch ? 1 : 0;

  const adjustInputHeight = useCallback(() => {
    const el = inputRef.current;
    if (!el) return;
    el.style.height = "0px";
    const max = 6 * 20;
    const next = Math.min(el.scrollHeight, max);
    el.style.height = `${next}px`;
  }, []);

  const handleInputKeyDown = useCallback(
    (e: any) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        if (!sendDisabled) {
          handleSendWithContext();
        }
      }
    },
    [handleSendWithContext, sendDisabled]
  );

  const handleInputInput = useCallback(() => {
    adjustInputHeight();
  }, [adjustInputHeight]);

  useEffect(() => {
    if (!modeMenuOpen) {
      setModeMenuRect(null);
      return;
    }
    const el = modeTriggerRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    setModeMenuRect({ bottom: window.innerHeight - rect.top + 6, left: rect.left });
  }, [modeMenuOpen]);

  useEffect(() => {
    if (!modeMenuOpen) return;
    const onPointer = (e: MouseEvent) => {
      const el = modeTriggerRef.current;
      const target = e.target as Node;
      if (el?.contains(target)) return;
      const menu = document.getElementById("agent-mode-menu");
      if (menu?.contains(target)) return;
      setModeMenuOpen(false);
    };
    window.addEventListener("mousedown", onPointer, true);
    return () => window.removeEventListener("mousedown", onPointer, true);
  }, [modeMenuOpen]);

  return (
    <div
      className={`ai-panel flex flex-col h-full text-xs text-[var(--text)] px-3 py-2 max-w-full box-border ${
        theme === "light" ? "ai-panel-theme-light" : "ai-panel-theme-dark"
      }`}
    >
      <div className="shrink-0 flex items-center justify-between gap-2 mb-2 rounded-lg border border-border bg-[var(--panel-muted-bg)] px-2 py-1.5">
        <span className="text-[11px] font-semibold tracking-wide text-[var(--text)]">{t("agent")}</span>
        <span className="rounded-full border border-border px-2 py-0.5 text-[10px] text-[var(--text-muted)]">
          {getAgentModeLabel(agentMode)}
        </span>
      </div>

      {hasConversation ? (
        <div className="flex-1 min-h-0 overflow-auto scroll-vscode space-y-2 pr-1">
          {messages.map((m, idx) => {
            const isAssistant = m.role === "assistant";
            const isLastAssistant =
              isAssistant && messages.slice(idx + 1).every((next) => next.role !== "assistant");
            const content = isAssistant ? m.text : m.text;
            return (
              <div
                key={m.id}
                className={`agent-msg ${
                  isAssistant ? "agent-msg-assistant" : "agent-msg-user"
                }`}
              >
                {isAssistant ? (
                  <>
                    <div
                      className="ai-md text-xs break-words"
                      dangerouslySetInnerHTML={{ __html: renderMarkdownToHtml(content) }}
                    />
                    {isLastAssistant && diff && agentMode !== "iterate" && (
                      <div className="agent-inline-diff-wrapper">
                        {renderInlineDiff(diff)}
                      </div>
                    )}
                    {isLastAssistant && iterationDiff && agentMode === "iterate" && (
                      <div className="mt-2">
                        <IterateDiffPreview diff={iterationDiff} defaultExpanded />
                      </div>
                    )}
                    {isLastAssistant && projectDir && onCreateMoFile && (
                      <div className="mt-2 flex flex-wrap items-center gap-2">
                        {newFileFromDiff && (
                          <button
                            type="button"
                            className="agent-filebar-btn agent-filebar-primary"
                            disabled={createFileLoading}
                            onClick={handleCreateFileFromDiff}
                          >
                            {createFileLoading ? "..." : t("aiCreateFile")}
                          </button>
                        )}
                        {!newFileFromDiff && codeBlock?.content && (
                          <button
                            type="button"
                            className="agent-filebar-btn agent-filebar-primary"
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
                ) : (
                  <div className="text-xs whitespace-pre-wrap break-words">{content}</div>
                )}
              </div>
            );
          })}
          {aiLoading && (
            <div className="agent-msg agent-msg-assistant agent-msg-loading">
              <span className="agent-thinking-dot" />
              <span className="agent-thinking-dot" />
              <span className="agent-thinking-dot" />
            </div>
          )}
        </div>
      ) : (
        <div className="flex-1 min-h-0 flex items-center justify-center text-[var(--text-muted)] text-xs text-center px-6">
          <div>
            <div className="mb-1">
              {t("aiAgentEmptyTitle")}
            </div>
            <div className="opacity-80">
              {t("aiAgentEmptyHint")}
            </div>
          </div>
        </div>
      )}
      {(currentFilePath || selectionPreview || lastJitErrorText) && (
        <div className="agent-status-row mt-2 text-[10px] text-[var(--text-muted)] flex items-center gap-3">
          {currentFilePath && (
            <span className="truncate max-w-[45%]">
              {t("aiCurrentFile")}: {currentFilePath}
            </span>
          )}
          {selectionPreview && <span>{t("selection")}</span>}
          {lastJitErrorText && lastJitErrorText.trim() && (
            <span className="agent-status-error">{t("jitError")}</span>
          )}
        </div>
      )}

      {lastJitErrorText && lastJitErrorText.trim() && agentMode !== "iterate" && (
        <div className="mt-2 rounded-lg border theme-banner-warning px-3 py-3 text-xs">
          <div className="font-medium mb-1">{t("compilationFailed")}</div>
          <div className="break-words opacity-90">{lastJitErrorText}</div>
          <div className="mt-3 flex flex-wrap gap-2">
            <button
              type="button"
              className="agent-filebar-btn agent-filebar-primary"
              onClick={() => {
                setAgentMode?.("edit-selection");
                setAiPrompt(`Fix this Modelica compile error:\n${lastJitErrorText}`);
              }}
            >
              {t("aiFixMyCode")}
            </button>
            <button
              type="button"
              className="agent-filebar-btn"
              onClick={() => {
                setAgentMode?.("iterate");
                setAiPrompt(`Extend the compiler to support this failing case:\n${lastJitErrorText}`);
              }}
            >
              {t("aiExtendCompiler")}
            </button>
          </div>
        </div>
      )}

      {pendingPatch?.newContent && (
        <div className="agent-filebar mt-2">
          <div className="agent-filebar-header">
            <div className="agent-filebar-left">
              <span className="agent-filebar-count">
                {fileCount === 1 ? tf("aiPendingFilesOne", { count: fileCount }) : tf("aiPendingFilesOther", { count: fileCount })}
              </span>
              <span className="agent-filebar-range">
                {pendingPatch.filePath || currentFilePath || t("aiCurrentFile")}
                {pendingPatch.startLine != null && pendingPatch.endLine != null
                  ? ` · ${tf("linesRange", { start: pendingPatch.startLine, end: pendingPatch.endLine })}`
                  : ` · ${t("selection")}`}
              </span>
            </div>
            <div className="agent-filebar-right">
              {aiLoading && (
                <button
                  type="button"
                  className="agent-filebar-btn"
                  disabled={!aiLoading}
                >
                  {t("aiStop")}
                </button>
              )}
              <button
                type="button"
                className="agent-filebar-btn agent-filebar-primary"
                onClick={onInsert}
              >
                {t("aiReview")}
              </button>
              <button
                type="button"
                className="agent-filebar-toggle"
                onClick={() => setFileBarOpen((v) => !v)}
              >
                {fileBarOpen ? "▾" : "▴"}
              </button>
            </div>
          </div>
          {fileBarOpen && (
            <pre className="agent-filebar-body">
              {pendingPatch.newContent}
            </pre>
          )}
        </div>
      )}

      {agentMode === "iterate" && (iterationDiff || iterationRunResult || iterationHistory.length > 0) && (
        <div className="mt-2 space-y-2">
          {(iterationDiff || iterationRunResult) && (
            <div className="rounded-lg border border-border bg-[var(--panel-muted-bg)] p-3">
              <div className="text-[11px] font-medium text-[var(--text)] mb-2">{t("compilerIteration")}</div>
              <IterateActions
                runLoading={iterationRunLoading}
                adoptLoading={iterationAdoptLoading}
                commitLoading={iterationCommitLoading}
                canRunFull={canRunFull}
                canAdopt={canAdopt}
                canCommit={canCommit}
                commitMessage={iterationCommitMessage}
                onCommitMessageChange={setIterationCommitMessage}
                onRunQuick={async () => {
                  if (!onRunIteration) return;
                  setIterationRunLoading(true);
                  try {
                    await onRunIteration(true);
                  } finally {
                    setIterationRunLoading(false);
                  }
                }}
                onRunFull={async () => {
                  if (!onRunIteration) return;
                  setIterationRunLoading(true);
                  try {
                    await onRunIteration(false);
                  } finally {
                    setIterationRunLoading(false);
                  }
                }}
                onAdopt={async () => {
                  if (!onAdoptIteration) return;
                  setIterationAdoptLoading(true);
                  try {
                    await onAdoptIteration();
                  } finally {
                    setIterationAdoptLoading(false);
                  }
                }}
                onCommit={async () => {
                  if (!onCommitIteration) return;
                  setIterationCommitLoading(true);
                  try {
                    await onCommitIteration(iterationCommitMessage);
                  } finally {
                    setIterationCommitLoading(false);
                  }
                }}
              />
            </div>
          )}

          {iterationHistory.length > 0 && onReuseIteration && (
            <IterateHistory
              history={iterationHistory}
              onReuseDiff={async (record) => {
                await onReuseIteration(record);
              }}
            />
          )}
        </div>
      )}

      <div className="mt-2 border border-border rounded-xl bg-[var(--agent-input-shell-bg)] overflow-hidden">
        <div className="px-3 pt-2 pb-1">
          <textarea
            ref={inputRef}
            placeholder={t("aiInputPlaceholder")}
            value={aiPrompt}
            onChange={(e) => setAiPrompt(e.target.value)}
            onKeyDown={handleInputKeyDown}
            onInput={handleInputInput}
            className="w-full bg-transparent text-sm resize-none text-[var(--text)] placeholder:text-[var(--text-muted)] outline-none agent-input-textarea"
            rows={1}
          />
        </div>

        <div className="px-3 pb-2 border-t border-border flex items-center justify-between gap-2 flex-wrap text-[10px]">
          <div className="flex items-center gap-2">
            <div className="relative">
              <button
                ref={modeTriggerRef}
                type="button"
                onClick={() => setModeMenuOpen((v) => !v)}
                className="inline-flex items-center gap-1 px-2 py-1 rounded border border-[var(--input-border)] hover:bg-[var(--surface-hover)] text-[var(--text)]"
              >
                <span>{getAgentModeLabel(agentMode)}</span>
                <span>▾</span>
              </button>
              {modeMenuOpen &&
                modeMenuRect &&
                createPortal(
                  <div
                    id="agent-mode-menu"
                    className="agent-mode-menu-portal"
                    style={{
                      position: "fixed",
                      bottom: modeMenuRect.bottom,
                      left: modeMenuRect.left,
                      zIndex: 10000,
                    }}
                  >
                    {(["explain", "edit-selection", "edit-file", "generate", "iterate"] as const).map((m) => (
                      <button
                        key={m}
                        type="button"
                        className="flex items-center gap-1 px-2 py-1 text-[10px] w-full hover:bg-[var(--surface-hover)] text-[var(--text)] text-left"
                        onClick={() => {
                          if (setAgentMode) {
                            setAgentMode(m);
                          } else {
                            setMode(m === "explain" ? "chat" : "code");
                          }
                          setModeMenuOpen(false);
                        }}
                      >
                        {getAgentModeLabel(m)}
                      </button>
                    ))}
                  </div>,
                  document.body
                )}
            </div>
            <select
              value={model}
              onChange={(e) => setModel(e.target.value)}
              className="bg-transparent border border-[var(--input-border)] px-2 py-1 rounded text-[var(--text)]"
            >
              <option value="deepseek-chat">deepseek</option>
            </select>
          </div>
          <div className="flex items-center gap-2">
            {(projectDir || repoRoot) && (
              <button
                type="button"
                onClick={() => setUseContext((v) => !v)}
                className={`agent-ctx-toggle ${useContext ? "agent-ctx-toggle-on" : ""}`}
                title={t("useIndexContext")}
              >
                <AppIcon name="index" aria-hidden="true" className="w-3 h-3" />
                {contextChunks.length > 0 && (
                  <span className="agent-ctx-count">{contextChunks.length}</span>
                )}
                {contextLoading && <span className="agent-ctx-loading" />}
              </button>
            )}
            <button
              type="button"
              onClick={handleSendWithContext}
              disabled={sendDisabled}
              className="inline-flex items-center justify-center h-7 w-7 rounded-full border border-[var(--input-border)] hover:bg-[var(--surface-hover)] text-[var(--text)] disabled:opacity-40 disabled:cursor-default"
              title={t("run")}
            >
              &gt;
            </button>
          </div>
        </div>
      </div>

      <style>{`
        .ai-md { word-wrap: break-word; overflow-wrap: anywhere; }
        .ai-md .ai-md-h { margin: 0 0 6px 0; font-size: 12px; font-weight: 600; }
        .ai-md .ai-md-p { margin: 0 0 6px 0; line-height: 1.4; white-space: pre-wrap; }
        .ai-md .ai-md-ul { margin: 0 0 6px 16px; padding: 0; list-style: disc; }
        .ai-md .ai-md-li { margin: 0 0 2px 0; }
        .ai-md .ai-md-inline { background: var(--agent-inline-code-bg); border: 1px solid var(--agent-inline-code-border); padding: 0 4px; border-radius: 4px; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace; font-size: 11px; }
        .ai-md .ai-md-pre { margin: 0 0 8px 0; background: var(--agent-code-bg); border: 1px solid var(--agent-code-border); padding: 8px; border-radius: 6px; overflow: auto; }
        .ai-md .ai-md-code { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace; font-size: 11px; }
        .ai-md .ai-md-spacer { height: 6px; }
      `}</style>
    </div>
  );
}
