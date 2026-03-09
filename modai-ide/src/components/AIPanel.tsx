import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";

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

interface AIPanelProps {
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
}

export function AIPanel({
  apiKey,
  setApiKey,
  apiKeySaved,
  onSaveApiKey,
  aiPrompt,
  setAiPrompt,
  aiLoading,
  aiResponse,
  onSend,
  onInsert,
  tokenEstimate,
  dailyTokenUsed,
  dailyTokenLimit,
  sendDisabled,
  projectDir,
}: AIPanelProps) {
  const [contextChunks, setContextChunks] = useState<ChunkInfo[]>([]);
  const [contextLoading, setContextLoading] = useState(false);
  const [useContext, setUseContext] = useState(true);
  const [showContext, setShowContext] = useState(false);

  const fetchContext = useCallback(async () => {
    if (!projectDir || !aiPrompt.trim()) {
      setContextChunks([]);
      return;
    }
    setContextLoading(true);
    try {
      const chunks = (await invoke("index_get_context", {
        projectDir,
        query: aiPrompt.trim(),
        maxChunks: 8,
      })) as ChunkInfo[];
      setContextChunks(chunks);
    } catch {
      setContextChunks([]);
    } finally {
      setContextLoading(false);
    }
  }, [projectDir, aiPrompt]);

  const handleSendWithContext = useCallback(() => {
    if (useContext && projectDir && aiPrompt.trim()) {
      fetchContext().then(() => {
        onSend();
      });
    } else {
      onSend();
    }
  }, [useContext, projectDir, aiPrompt, fetchContext, onSend]);

  return (
    <>
      <div className="text-sm font-medium text-[var(--text-muted)] mb-2">
        {t("aiCoding")}
      </div>
      {!apiKeySaved ? (
        <div className="flex gap-2 mb-2">
          <input
            type="password"
            placeholder="DeepSeek API key"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            className="flex-1 bg-[#3c3c3c] border border-gray-600 px-2 py-1 text-sm rounded"
          />
          <button
            type="button"
            onClick={() => onSaveApiKey(apiKey)}
            className="px-2 py-1 bg-primary hover:bg-blue-600 text-sm shrink-0 rounded"
          >
            Save
          </button>
        </div>
      ) : (
        <div className="text-xs text-[var(--text-muted)] mb-2">
          {t("apiKeySaved")}
        </div>
      )}
      <div className="text-xs text-[var(--text-muted)] mb-1">
        {t("tokenEstimate")}: {tokenEstimate} &middot; {t("dailyUsed")}:{" "}
        {dailyTokenUsed} / {dailyTokenLimit}
      </div>
      <textarea
        placeholder="e.g. Generate a Modelica model: first-order system with time constant 1"
        value={aiPrompt}
        onChange={(e) => setAiPrompt(e.target.value)}
        className="w-full h-20 bg-[#3c3c3c] border border-gray-600 px-2 py-1 text-sm resize-none mb-2 rounded"
        rows={3}
      />

      {projectDir && (
        <div className="flex items-center gap-2 mb-2 text-xs">
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
        <div className="mb-2 max-h-40 overflow-auto border border-gray-700 rounded bg-[#1e1e1e] p-1.5">
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

      <div className="flex gap-2 mb-2">
        <button
          type="button"
          onClick={handleSendWithContext}
          disabled={sendDisabled}
          className="px-2 py-1 bg-primary hover:bg-blue-600 text-sm disabled:opacity-50 rounded"
        >
          {aiLoading ? "..." : "Send"}
        </button>
        {useContext && projectDir && (
          <button
            type="button"
            onClick={fetchContext}
            disabled={!aiPrompt.trim() || contextLoading}
            className="px-2 py-1 bg-gray-600 hover:bg-gray-500 text-sm disabled:opacity-50 rounded"
          >
            {t("fetchContext") || "Fetch Context"}
          </button>
        )}
        {aiResponse && (
          <button
            type="button"
            onClick={onInsert}
            className="px-2 py-1 bg-gray-600 hover:bg-gray-500 text-sm rounded"
          >
            Insert
          </button>
        )}
      </div>
      {aiResponse && (
        <pre className="flex-1 min-h-0 overflow-auto text-xs bg-[#1e1e1e] p-2 whitespace-pre-wrap border border-gray-700 rounded">
          {aiResponse}
        </pre>
      )}
    </>
  );
}
