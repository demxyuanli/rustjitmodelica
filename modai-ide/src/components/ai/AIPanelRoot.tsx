import { useState, useCallback, useEffect, useRef } from "react";
import { indexRepoGetContext, indexGetContext } from "../../api/tauri";
import type { IterationRecord, IterationRunResult } from "../../api/tauri";
import type { AgentMode, AiContextBlock, PendingPatch } from "../../hooks/useAI";
import type { ChatMessage, ChunkInfo } from "./ai-markdown";

import { AIChatHeader } from "./AIChatHeader";
import { AIChatMessages } from "./AIChatMessages";
import { AIChatEmptyState } from "./AIChatEmptyState";
import { AIChatInput } from "./AIChatInput";
import { AIChatStatusBar } from "./AIChatStatusBar";
import { AIChatErrorBanner } from "./AIChatErrorBanner";
import { AIChatPendingPatch } from "./AIChatPendingPatch";
import { AIChatIterationSection } from "./AIChatIterationSection";

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
  onApplyDiff?: (diff: string) => Promise<void>;
  iterationDiff?: string | null;
  iterationRunResult?: IterationRunResult | null;
  iterationHistory?: IterationRecord[];
  onRunIteration?: (quick: boolean) => Promise<unknown>;
  onAdoptIteration?: () => Promise<unknown>;
  onCommitIteration?: (message?: string) => Promise<unknown>;
  onReuseIteration?: (record: IterationRecord) => Promise<unknown>;
  theme?: "dark" | "light";
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
  dailyTokenUsed,
  dailyTokenLimit,
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
  onApplyDiff,
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
  const [localMessages, setLocalMessages] = useState<ChatMessage[]>([]);
  const lastAssistantRef = useRef<string | null>(null);

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

  useEffect(() => {
    if (!aiResponse || messagesProp !== undefined) return;
    const text = aiResponse;
    if (!text.trim()) return;
    if (lastAssistantRef.current === text) return;
    lastAssistantRef.current = text;
    setLocalMessages((prev) => [...prev, { id: Date.now(), role: "assistant", text }]);
  }, [aiResponse, messagesProp]);

  const handleNewChat = useCallback(() => {
    setLocalMessages([]);
    lastAssistantRef.current = null;
    setAiPrompt("");
  }, [setAiPrompt]);

  const hasConversation = messages.length > 0;

  return (
    <div
      className={`ai-panel flex flex-col h-full text-xs text-[var(--text)] max-w-full box-border ${
        theme === "light" ? "ai-panel-theme-light" : "ai-panel-theme-dark"
      }`}
    >
      <AIChatHeader
        agentMode={agentMode}
        hasMessages={hasConversation}
        onNewChat={handleNewChat}
      />

      {hasConversation ? (
        <AIChatMessages
          messages={messages}
          aiLoading={aiLoading}
          agentMode={agentMode}
          iterationDiff={iterationDiff}
          projectDir={projectDir}
          onCreateMoFile={onCreateMoFile}
          onRegenerate={handleSendWithContext}
          onApplyDiff={onApplyDiff}
        />
      ) : (
        <AIChatEmptyState
          setAgentMode={setAgentMode}
          setAiPrompt={setAiPrompt}
        />
      )}

      <AIChatStatusBar
        currentFilePath={currentFilePath}
        currentSelectionText={currentSelectionText}
        lastJitErrorText={lastJitErrorText}
      />

      {lastJitErrorText && (
        <AIChatErrorBanner
          lastJitErrorText={lastJitErrorText}
          agentMode={agentMode}
          setAgentMode={setAgentMode}
          setAiPrompt={setAiPrompt}
        />
      )}

      {pendingPatch?.newContent && (
        <AIChatPendingPatch
          pendingPatch={pendingPatch}
          currentFilePath={currentFilePath}
          aiLoading={aiLoading}
          onInsert={onInsert}
        />
      )}

      {agentMode === "iterate" && (
        <AIChatIterationSection
          iterationDiff={iterationDiff}
          iterationRunResult={iterationRunResult}
          iterationHistory={iterationHistory}
          onRunIteration={onRunIteration}
          onAdoptIteration={onAdoptIteration}
          onCommitIteration={onCommitIteration}
          onReuseIteration={onReuseIteration}
        />
      )}

      <AIChatInput
        aiPrompt={aiPrompt}
        setAiPrompt={setAiPrompt}
        sendDisabled={sendDisabled}
        aiLoading={aiLoading}
        agentMode={agentMode}
        setAgentMode={setAgentMode}
        model={model}
        setModel={setModel}
        setMode={setMode}
        useContext={useContext}
        setUseContext={setUseContext}
        contextChunks={contextChunks}
        contextLoading={contextLoading}
        projectDir={projectDir}
        repoRoot={repoRoot}
        dailyTokenUsed={dailyTokenUsed}
        dailyTokenLimit={dailyTokenLimit}
        onSend={handleSendWithContext}
      />
    </div>
  );
}
