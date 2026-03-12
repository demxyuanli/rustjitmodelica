import { useMemo, useState, useCallback, useEffect, useRef } from "react";
import { indexRepoGetContext, indexGetContext } from "../api/tauri";
import { t } from "../i18n";
import { AppIcon } from "./Icon";

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

export interface AIPanelProps {
  apiKey: string;
  setApiKey: (v: string) => void;
  apiKeySaved: boolean;
  onSaveApiKey: (key: string) => void;
  aiPrompt: string;
  setAiPrompt: (v: string) => void;
  aiLoading: boolean;
  aiResponse: string | null;
  onSend: () => void;
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
  mode,
  setMode,
  model,
  setModel,
}: AIPanelProps) {
  const [contextChunks, setContextChunks] = useState<ChunkInfo[]>([]);
  const [contextLoading, setContextLoading] = useState(false);
  const [useContext, setUseContext] = useState(true);
  const [showContext, setShowContext] = useState(false);
  const [modeMenuOpen, setModeMenuOpen] = useState(false);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const lastAssistantRef = useRef<string | null>(null);

  const fetchContext = useCallback(async () => {
    if (!aiPrompt.trim()) {
      setContextChunks([]);
      return;
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
        setContextLoading(false);
        return;
      }
      setContextChunks(chunks);
    } catch {
      setContextChunks([]);
    } finally {
      setContextLoading(false);
    }
  }, [projectDir, aiPrompt]);

  const handleSendWithContext = useCallback(() => {
    const promptText = aiPrompt.trim();
    if (!promptText) return;
    setMessages((prev) => [
      ...prev,
      {
        id: Date.now(),
        role: "user",
        text: promptText,
      },
    ]);
    if (useContext && (projectDir || repoRoot)) {
      fetchContext().then(() => {
        onSend();
      });
    } else {
      onSend();
    }
  }, [aiPrompt, useContext, projectDir, repoRoot, fetchContext, onSend]);

  useEffect(() => {
    if (!aiResponse) return;
    const text = aiResponse;
    if (!text.trim()) return;
    if (lastAssistantRef.current === text) return;
    lastAssistantRef.current = text;
    setMessages((prev) => [
      ...prev,
      {
        id: Date.now(),
        role: "assistant",
        text,
      },
    ]);
  }, [aiResponse]);

  const lastAnswerText = useMemo(() => {
    for (let i = messages.length - 1; i >= 0; i -= 1) {
      if (messages[i]?.role === "assistant") return messages[i]?.text ?? "";
    }
    return aiResponse ?? "";
  }, [messages, aiResponse]);

  const { answer, diff } = useMemo(() => splitAnswerAndDiff(lastAnswerText), [lastAnswerText]);
  const answerHtml = useMemo(() => renderMarkdownToHtml(answer), [answer]);
  const hasConversation = useMemo(
    () => !!(lastAnswerText && lastAnswerText.trim()),
    [lastAnswerText],
  );

  return (
    <div className="flex flex-col h-full text-xs text-[var(--text)] px-3 py-2">
      {hasConversation ? (
        <div className="flex-1 min-h-0 overflow-auto scroll-vscode space-y-2">
          <div className="rounded border border-gray-700 bg-[#111111] p-2">
            <div className="text-[10px] uppercase tracking-wider text-[var(--text-muted)] mb-1">
              {t("aiAnswer") || "Answer"}
            </div>
            {answer ? (
              <div
                className="ai-md text-xs text-[var(--text)]"
                // rendered from local markdown-to-html (no external scripts)
                dangerouslySetInnerHTML={{ __html: answerHtml }}
              />
            ) : (
              <div className="text-[var(--text-muted)] text-xs">
                {aiLoading ? (t("loading") || "Loading") : ""}
              </div>
            )}
          </div>

          <div className="rounded border border-gray-700 bg-[#111111] overflow-hidden">
            <div className="px-2 py-1 border-b border-gray-700 bg-[#1b1b1b] text-[var(--text)] flex items-center justify-between gap-2">
              <span>{t("aiCodeDiff") || "code diff"}</span>
              {diff && (
                <button
                  type="button"
                  onClick={onInsert}
                  className="inline-flex items-center justify-center h-6 w-6 rounded-full border border-gray-600 hover:bg-white/5 text-[var(--text)]"
                  title="Insert into editor"
                >
                  <AppIcon name="diff" aria-hidden="true" className="w-3 h-3" />
                </button>
              )}
            </div>
            <pre className="p-2 text-[10px] text-[var(--text-muted)] whitespace-pre-wrap min-h-[140px]">
              {diff || ""}
            </pre>
          </div>
        </div>
      ) : (
        <div className="flex-1 min-h-0" />
      )}

      {projectDir && (
        <div className="flex items-center gap-2 mt-2 text-xs">
          <label className="flex items-center gap-1 cursor-pointer text-[var(--text-muted)]">
            <input
              type="checkbox"
              checked={useContext}
              onChange={(e) => setUseContext(e.target.checked)}
              className="shrink-0"
            />
            {t("useIndexContext") || "Use code context"}
          </label>
          {contextChunks.length > 0 && (
            <button
              type="button"
              className="text-[var(--accent)] hover:underline"
              onClick={() => setShowContext((s) => !s)}
            >
              {showContext
                ? t("hideContext") || "Hide"
                : `${t("showContext") || "Show"} (${contextChunks.length})`}
            </button>
          )}
          {contextLoading && (
            <span className="text-[var(--text-muted)]">
              {t("loadingContext") || "Loading..."}
            </span>
          )}
        </div>
      )}

      {showContext && contextChunks.length > 0 && (
        <div className="mt-2 max-h-32 overflow-auto border border-gray-700 rounded bg-[#111111] p-1.5">
          {contextChunks.map((ch) => (
            <div key={ch.id} className="mb-1 last:mb-0">
              <div className="text-[10px] text-[var(--accent)] font-mono">
                {ch.filePath}:{ch.lineStart}-{ch.lineEnd}
                {ch.contextLabel && (
                  <span className="text-[var(--text-muted)] ml-1">
                    ({ch.contextLabel})
                  </span>
                )}
              </div>
              <pre className="text-[10px] text-[var(--text-muted)] whitespace-pre-wrap max-h-16 overflow-hidden">
                {ch.content.slice(0, 300)}
                {ch.content.length > 300 ? "..." : ""}
              </pre>
            </div>
          ))}
        </div>
      )}

      <div className="mt-2 border border-gray-700 rounded-lg bg-transparent overflow-hidden">
        <div className="px-3 py-2">
          <textarea
            placeholder={t("aiInputPlaceholder") || "Input text"}
            value={aiPrompt}
            onChange={(e) => setAiPrompt(e.target.value)}
            className="w-full bg-transparent text-sm resize-none min-h-[84px] text-[var(--text)] placeholder:text-[var(--text-muted)] outline-none"
            rows={4}
          />
        </div>

        <div className="px-3 pb-2 flex items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <div className="relative">
              <button
                type="button"
                onClick={() => setModeMenuOpen((v) => !v)}
                className="inline-flex items-center gap-1 px-2 py-1 rounded border border-gray-600 hover:bg-white/5 text-[var(--text)]"
              >
                <span className="text-[10px]">{mode === "code" ? "code" : "chat"}</span>
                <span className="text-[10px]">▾</span>
              </button>
              {modeMenuOpen && (
                <div className="absolute z-10 left-0 bottom-full mb-1 rounded border border-gray-700 bg-[#1e1e1e] shadow-lg min-w-[80px]">
                  <button
                    type="button"
                    className="flex items-center gap-1 px-2 py-1 text-[10px] w-full hover:bg-white/10 text-[var(--text)]"
                    onClick={() => {
                      setMode("code");
                      setModeMenuOpen(false);
                    }}
                  >
                    <span>code</span>
                  </button>
                  <button
                    type="button"
                    className="flex items-center gap-1 px-2 py-1 text-[10px] w-full hover:bg-white/10 text-[var(--text)]"
                    onClick={() => {
                      setMode("chat");
                      setModeMenuOpen(false);
                    }}
                  >
                    <span>chat</span>
                  </button>
                </div>
              )}
            </div>

            <div className="flex items-center gap-1">
              <select
                value={model}
                onChange={(e) => setModel(e.target.value)}
                className="bg-transparent border border-gray-600 px-2 py-1 text-[10px] rounded text-[var(--text)]"
              >
                <option value="deepseek-chat">deepseek</option>
              </select>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={handleSendWithContext}
              disabled={sendDisabled}
              className="inline-flex items-center justify-center h-7 w-7 rounded-full border border-gray-600 hover:bg-white/5 text-[var(--text)] disabled:opacity-40 disabled:cursor-default"
              title={t("run") || "Run"}
            >
              &gt;
            </button>
          </div>
        </div>
      </div>

      <style>{`
        .ai-md .ai-md-h { margin: 0 0 6px 0; font-size: 12px; font-weight: 600; }
        .ai-md .ai-md-p { margin: 0 0 6px 0; line-height: 1.4; }
        .ai-md .ai-md-ul { margin: 0 0 6px 16px; padding: 0; list-style: disc; }
        .ai-md .ai-md-li { margin: 0 0 2px 0; }
        .ai-md .ai-md-inline { background: rgba(255,255,255,0.06); border: 1px solid rgba(255,255,255,0.12); padding: 0 4px; border-radius: 4px; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace; font-size: 11px; }
        .ai-md .ai-md-pre { margin: 0 0 8px 0; background: rgba(255,255,255,0.04); border: 1px solid rgba(255,255,255,0.12); padding: 8px; border-radius: 6px; overflow: auto; }
        .ai-md .ai-md-code { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace; font-size: 11px; }
        .ai-md .ai-md-spacer { height: 6px; }
      `}</style>
    </div>
  );
}
