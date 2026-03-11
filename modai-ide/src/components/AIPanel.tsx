import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
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
  aiLoading: _aiLoading,
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
  currentFilePath,
  currentSelectionText,
  lastJitErrorText,
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
        chunks = (await invoke("index_repo_get_context", {
          query: aiPrompt.trim(),
          maxChunks: 8,
        })) as ChunkInfo[];
      } else if (projectDir) {
        chunks = (await invoke("index_get_context", {
          projectDir,
          query: aiPrompt.trim(),
          maxChunks: 8,
        })) as ChunkInfo[];
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

  return (
    <div className="flex flex-col h-full text-xs text-[var(--text)] rounded-lg border border-border bg-[#1e1e1e] px-3 py-2">
      <div className="flex flex-wrap gap-1 mb-2 text-[10px] text-[var(--text-muted)]">
        {currentSelectionText && (
          <button
            type="button"
            className="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-[#2a2a2a] hover:bg-[#333]"
            onClick={() => {
              const base = "Explain and improve the following selection.\n\n";
              setAiPrompt(base + currentSelectionText);
            }}
          >
            <AppIcon name="explorer" aria-hidden="true" className="w-3 h-3" />
            <span>Explain selection</span>
          </button>
        )}
        {currentSelectionText && (
          <button
            type="button"
            className="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-[#2a2a2a] hover:bg-[#333]"
            onClick={() => {
              const base = "Refactor the following Modelica code for clarity and robustness. Keep behavior equivalent.\n\n";
              setAiPrompt(base + currentSelectionText);
            }}
          >
            <AppIcon name="run" aria-hidden="true" className="w-3 h-3" />
            <span>Refactor selection</span>
          </button>
        )}
        {lastJitErrorText && (
          <button
            type="button"
            className="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-[#2a2a2a] hover:bg-[#333]"
            onClick={() => {
              const base = "Fix the following Modelica compile error and propose corrected code.\n\n";
              setAiPrompt(base + lastJitErrorText);
            }}
          >
            <AppIcon name="error" aria-hidden="true" className="w-3 h-3" />
            <span>Fix compile error</span>
          </button>
        )}
        {currentFilePath && (
          <button
            type="button"
            className="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-[#2a2a2a] hover:bg-[#333]"
            onClick={() => {
              const base = `Review this file and suggest improvements.\nFile: ${currentFilePath}\n\n`;
              setAiPrompt(base);
            }}
          >
            <AppIcon name="sourceControl" aria-hidden="true" className="w-3 h-3" />
            <span>Review file</span>
          </button>
        )}
      </div>
      <div className="flex-1 min-h-0 overflow-auto mb-2 scroll-vscode">
        {messages.map((m) => (
          <div
            key={m.id}
            className={`mb-1 flex ${m.role === "user" ? "justify-end" : "justify-start"}`}
          >
            <div
              className={`max-w-full text-xs whitespace-pre-wrap px-2 py-1 rounded ${
                m.role === "user"
                  ? "bg-[#2a2a2a]"
                  : "bg-[#111111] border border-gray-700"
              }`}
            >
              {m.text}
            </div>
          </div>
        ))}
      </div>

      <div className="mt-2 pt-2 border-t border-gray-700">
        <textarea
          placeholder="e.g. Generate a Modelica model: first-order system with time constant 1"
          value={aiPrompt}
          onChange={(e) => setAiPrompt(e.target.value)}
          className="w-full bg-transparent px-1 py-1 text-sm resize-none min-h-[72px] text-[var(--text)] placeholder:text-[var(--text-muted)] outline-none"
          rows={3}
        />
      </div>

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

      <div className="mt-2 flex items-center justify-between gap-2">
        <div className="inline-flex items-center gap-2 rounded bg-[#252526] px-2 py-0.5 text-[var(--text-muted)]">
          <div className="relative">
            <button
              type="button"
              onClick={() => setModeMenuOpen((v) => !v)}
              className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded hover:bg-white/5 text-[var(--text)]"
            >
              {mode === "code" ? (
                <AppIcon name="run" aria-hidden="true" className="w-3.5 h-3.5" />
              ) : (
                <AppIcon name="ai" aria-hidden="true" className="w-3.5 h-3.5" />
              )}
              <span className="text-[10px]">{mode === "code" ? "Code" : "Chat"}</span>
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
                  <AppIcon name="run" aria-hidden="true" className="w-3 h-3" />
                  <span>Code</span>
                </button>
                <button
                  type="button"
                  className="flex items-center gap-1 px-2 py-1 text-[10px] w-full hover:bg-white/10 text-[var(--text)]"
                  onClick={() => {
                    setMode("chat");
                    setModeMenuOpen(false);
                  }}
                >
                  <AppIcon name="ai" aria-hidden="true" className="w-3 h-3" />
                  <span>Chat</span>
                </button>
              </div>
            )}
          </div>
          <div className="flex items-center gap-1">
            <AppIcon name="language" aria-hidden="true" className="w-3 h-3 text-[var(--text-muted)]" />
            <select
              value={model}
              onChange={(e) => setModel(e.target.value)}
              className="bg-[#1e1e1e] border border-gray-600 px-1 py-0.5 text-[10px] rounded text-[var(--text)]"
            >
              <option value="deepseek-coder-v2">deepseek-coder-v2</option>
              <option value="deepseek-chat">deepseek-chat</option>
            </select>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={handleSendWithContext}
            disabled={sendDisabled}
            className="inline-flex items-center justify-center h-6 w-6 rounded-full bg-white/80 hover:bg-white text-[#111111] disabled:opacity-40 disabled:cursor-default"
          >
            <AppIcon name="run" aria-hidden="true" className="w-3 h-3" />
          </button>
          {aiResponse && (
            <button
              type="button"
              onClick={onInsert}
              className="inline-flex items-center justify-center h-6 w-6 rounded-full bg-[#3c3c3c] hover:bg-gray-500 text-[var(--text)]"
              title="Insert into editor"
            >
              <AppIcon name="diff" aria-hidden="true" className="w-3 h-3" />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
