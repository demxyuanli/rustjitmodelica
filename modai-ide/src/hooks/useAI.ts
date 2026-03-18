import { useState, useCallback, useEffect } from "react";
import {
  getApiKey,
  setApiKey as setApiKeyCommand,
  getGrokApiKey,
  aiCodeGenStream,
  aiGenerateCompilerPatch,
  aiGenerateCompilerPatchWithContext,
  applyPatchToWorkspace,
  commitIterationPatch,
  getIteration,
  listIterationHistory,
  runSelfIterate,
  saveIteration,
  type IterationRecord,
  type IterationRunResult,
} from "../api/tauri";
import { useAISessions } from "./useAISessions";
import { listen } from "@tauri-apps/api/event";

const DAILY_TOKEN_LIMIT = 50000;
const DEFAULT_MODEL = "deepseek-chat";

function estimateTokens(text: string): number {
  return Math.ceil(text.length * 1.2);
}

function getDailyUsed(): number {
  try {
    const raw = localStorage.getItem("modai-ai-daily");
    if (!raw) return 0;
    const { date, used } = JSON.parse(raw) as { date: string; used: number };
    const today = new Date().toISOString().slice(0, 10);
    return date === today ? used : 0;
  } catch {
    return 0;
  }
}

function setDailyUsedStorage(used: number): void {
  try {
    const date = new Date().toISOString().slice(0, 10);
    localStorage.setItem("modai-ai-daily", JSON.stringify({ date, used }));
  } catch { /* ignore */ }
}

export interface AiContextBlock {
  path: string;
  content: string;
  range?: { start: number; end: number };
}

export type AgentMode = "explain" | "edit-selection" | "edit-file" | "generate" | "iterate";

export interface AiMessage {
  id: number;
  role: "user" | "assistant";
  text: string;
}

export interface PendingPatch {
  filePath: string | null;
  startLine: number | null;
  endLine: number | null;
  newContent: string | null;
}

function buildIterateAssistantMessage(target: string, diff: string): string {
  return [
    `Prepared compiler patch for \`${target}\`. Review the diff and run the sandbox actions below.`,
    "",
    "```diff",
    diff,
    "```",
  ].join("\n");
}

type AiMode = "chat" | "code";

function parsePatchFromResponse(
  result: string,
  agentMode: AgentMode,
  activeFilePath: string | null
): PendingPatch | null {
  if (agentMode === "edit-selection") {
    const trimmed = result.trim();
    if (!trimmed) return null;
    return {
      filePath: activeFilePath,
      startLine: null,
      endLine: null,
      newContent: trimmed,
    };
  }
  if (agentMode === "edit-file") {
    const m = result.match(/```json\s*([\s\S]*?)```/i);
    if (!m) return null;
    try {
      const obj = JSON.parse(m[1].trim()) as { patches?: Array<{ filePath?: string; startLine?: number; endLine?: number; newContent?: string }> };
      const list = obj.patches;
      if (!Array.isArray(list) || list.length === 0) return null;
      const patch = list.find((p) => !p.filePath || p.filePath === activeFilePath || (activeFilePath && activeFilePath.endsWith(p.filePath ?? "")));
      if (!patch) return null;
      return {
        filePath: patch.filePath ?? activeFilePath,
        startLine: patch.startLine ?? null,
        endLine: patch.endLine ?? null,
        newContent: patch.newContent ?? null,
      };
    } catch {
      return null;
    }
  }
  return null;
}

function createAiHook(log: (msg: string) => void, kind: "modelica" | "jit") {
  const [projectDir, setProjectDir] = useState<string | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [apiKeySaved, setApiKeySaved] = useState(false);
  const [grokApiKeySaved, setGrokApiKeySaved] = useState(false);
  const [aiPrompt, setAiPrompt] = useState("");
  const [aiLoading, setAiLoading] = useState(false);
  const [aiResponse, setAiResponse] = useState<string | null>(null);
  const [dailyTokenUsed, setDailyTokenUsed] = useState(getDailyUsed);
  const [agentMode, setAgentMode] = useState<AgentMode>("edit-selection");
  const [messages, setMessages] = useState<AiMessage[]>([]);
  const [pendingPatch, setPendingPatch] = useState<PendingPatch | null>(null);
  const [activeFilePath, setActiveFilePath] = useState<string | null>(null);
  const [iterationDiff, setIterationDiff] = useState<string | null>(null);
  const [iterationRunResult, setIterationRunResult] = useState<IterationRunResult | null>(null);
  const [iterationHistory, setIterationHistory] = useState<IterationRecord[]>([]);
  const [model, setModel] = useState<string>(() => {
    try {
      const stored = localStorage.getItem("modai-ai-model");
      if (!stored) return DEFAULT_MODEL;
      if (stored === "deepseek-coder-v2") return DEFAULT_MODEL;
      return stored;
    } catch {
      return DEFAULT_MODEL;
    }
  });
  const [contextBlocks, setContextBlocks] = useState<AiContextBlock[]>([]);
  const [lastToolCallsUsed, setLastToolCallsUsed] = useState<string[] | null>(null);
  const streamingCancelRef = useState<{ token: number }>(() => ({ token: 0 }))[0];
  const streamReqRef = useState<{ id: string | null }>(() => ({ id: null }))[0];

  const { sessions, saveCurrentSession, loadSession, deleteSession } = useAISessions(projectDir);

  useEffect(() => {
    getApiKey()
      .then((k) => {
        setApiKey(k ? "********" : "");
        setApiKeySaved(!!k);
      })
      .catch(() => {});
    getGrokApiKey()
      .then((k) => setGrokApiKeySaved(!!k))
      .catch(() => {});
  }, []);

  useEffect(() => {
    setDailyTokenUsed(getDailyUsed());
  }, []);

  const loadIterationHistory = useCallback(async () => {
    try {
      setIterationHistory(await listIterationHistory(20));
    } catch {
      setIterationHistory([]);
    }
  }, []);

  useEffect(() => {
    loadIterationHistory();
  }, [loadIterationHistory]);

  const saveApiKey = useCallback(async (key: string) => {
    if (!key || key === "********") return;
    try {
      await setApiKeyCommand(key);
      setApiKeySaved(true);
      setApiKey("********");
      log("API key saved");
    } catch (e) {
      log("API key save error: " + String(e));
    }
  }, [log]);

  const send = useCallback(async (extraContextBlocks?: AiContextBlock[]) => {
    if (!aiPrompt.trim()) return;
    const basePrompt = aiPrompt.trim();
    const blocks = extraContextBlocks?.length ? [...contextBlocks, ...extraContextBlocks] : contextBlocks;
    if (agentMode === "iterate") {
      setAiLoading(true);
      setAiResponse(null);
      setPendingPatch(null);
      setIterationRunResult(null);
      setMessages((prev) => [...prev, { id: Date.now(), role: "user", text: basePrompt }]);
      try {
        const compilerContextFiles = [...new Set(blocks
          .map((block) => block.path.replace(/\\/g, "/"))
          .filter((path) => path.startsWith("src/") || path.endsWith(".rs")))];
        const testCases = [...new Set(blocks
          .map((block) => block.path.replace(/\\/g, "/"))
          .filter((path) => path.startsWith("TestLib/") && path.endsWith(".mo"))
          .map((path) => path.replace(/\.mo$/, "")))];
        const diff = compilerContextFiles.length > 0 || testCases.length > 0
          ? await aiGenerateCompilerPatchWithContext(basePrompt, compilerContextFiles, testCases)
          : await aiGenerateCompilerPatch(basePrompt);
        const assistantText = buildIterateAssistantMessage(basePrompt, diff);
        setIterationDiff(diff);
        setAiResponse(assistantText);
        setMessages((prev) => [...prev, { id: Date.now() + 1, role: "assistant", text: assistantText }]);
        log("Iteration patch prepared");
      } catch (e) {
        const text = "Error: " + String(e);
        setAiResponse(text);
        setMessages((prev) => [...prev, { id: Date.now() + 1, role: "assistant", text }]);
        log("AI error: " + String(e));
      } finally {
        setAiLoading(false);
      }
      return;
    }
    const contextText = blocks.map((b) => b.content).join("\n\n");
    const est = estimateTokens(basePrompt + "\n" + contextText);
    const used = getDailyUsed();
    if (used + est > DAILY_TOKEN_LIMIT) {
      log("Daily token limit reached. Used: " + used + ", limit: " + DAILY_TOKEN_LIMIT);
      return;
    }
    setAiLoading(true);
    setAiResponse(null);
    setPendingPatch(null);
    streamingCancelRef.token += 1;
    const userMsgId = Date.now();
    setMessages((prev) => [...prev, { id: userMsgId, role: "user", text: basePrompt }]);
    try {
      const modeTag = `[MODE:${agentMode}] `;
      const promptForModel = modeTag + basePrompt;

      let system: string | undefined;
      if (agentMode === "explain") {
        system = "You are an expert coding assistant. Explain or review the provided code. Reply in natural language only. Do not output code patches or diffs.";
      } else if (agentMode === "edit-selection") {
        system = kind === "jit"
          ? "You are an expert Rust JIT compiler assistant. The user prompt and Relevant code/context describe the change. Reply with ONLY the new code for the selected region. No explanation, no markdown, no diff."
          : "You are an expert Modelica IDE assistant. The user prompt and Relevant code/context describe the change. Reply with ONLY the new code for the selected region. No explanation, no markdown, no diff.";
      } else if (agentMode === "edit-file") {
        system = "You are an expert coding assistant. Reply with a single JSON object in a ```json code block. Format: { \"patches\": [ { \"filePath\": \"relative/path.mo\", \"startLine\": 1, \"endLine\": 10, \"newContent\": \"...\" } ] }. Only one patch for the current file.";
      } else {
        system = kind === "jit"
          ? "You are an expert Rust JIT compiler assistant. Generate the requested code. Prefer replying with code only when the user asks for generation."
          : "You are an expert Modelica IDE assistant. When the user asks to add or generate a new Modelica model or .mo file, reply with: (1) a brief explanation, then (2) a unified diff in a ```diff code block that adds the single new file. Example: ```diff\n--- /dev/null\n+++ b/TestLib/ModelName.mo\n@@ -0,0 +1,10 @@\n+model ModelName\n+  Real x(start=0);\n+equation\n+  der(x)=1;\n+end ModelName;\n``` Every line of the new file must be prefixed with + in the diff so the IDE can create the file. For other requests reply with code or explanation as appropriate.";
      }

      const payload = {
        prompt: promptForModel,
        system,
        contextBlocks: blocks.length > 0 ? blocks : undefined,
        projectDir: projectDir ?? undefined,
        options: {
          model: model || DEFAULT_MODEL,
        },
      };

      const requestId = `r-${Date.now()}-${Math.random().toString(16).slice(2)}`;
      streamReqRef.id = requestId;
      const asstId = Date.now();
      setAiResponse(null);
      setMessages((prev) => [...prev, { id: asstId, role: "assistant", text: "..." }]);

      let full = "";
      let toolCallsUsed: string[] | null = null;
      let firstDelta = true;

      const unlistenDelta = await listen<{ requestId: string; delta: string }>("ai-stream-delta", (e) => {
        if (e.payload?.requestId !== requestId) return;
        const delta = String(e.payload?.delta ?? "");
        if (!delta) return;
        full += delta;
        setMessages((prev) =>
          prev.map((m) => {
            if (m.id !== asstId) return m;
            const base = firstDelta ? "" : (m.text ?? "");
            firstDelta = false;
            return { ...m, text: base + delta };
          })
        );
      });

      const unlistenTool = await listen<{ requestId: string; stage: string; name: string }>("ai-stream-tool", (e) => {
        if (e.payload?.requestId !== requestId) return;
        const stage = String(e.payload?.stage ?? "");
        const name = String(e.payload?.name ?? "");
        if (!name) return;
        const line = stage === "start" ? `[tool start] ${name}` : `[tool end] ${name}`;
        setMessages((prev) =>
          prev.map((m) => (m.id === asstId ? { ...m, text: `${m.text ?? ""}\n${line}` } : m))
        );
      });

      let resolveDone: (() => void) | null = null;
      let rejectDone: ((err: Error) => void) | null = null;
      const donePromise = new Promise<void>((resolve, reject) => {
        resolveDone = resolve;
        rejectDone = reject;
      });

      const unlistenDone = await listen<{ requestId: string; content: string; toolCallsUsed?: string[] }>("ai-stream-done", (e) => {
        if (e.payload?.requestId !== requestId) return;
        toolCallsUsed = Array.isArray(e.payload?.toolCallsUsed) ? e.payload.toolCallsUsed : null;
        const content = String(e.payload?.content ?? "");
        if (!content && full) {
          // keep current
        } else if (content && content.length > full.length) {
          const extra = content.slice(full.length);
          full = content;
          setMessages((prev) =>
            prev.map((m) => (m.id === asstId ? { ...m, text: (m.text ?? "") + extra } : m))
          );
        } else if (content && content.length < full.length) {
          full = content;
          setMessages((prev) =>
            prev.map((m) => (m.id === asstId ? { ...m, text: content } : m))
          );
        } else if (!content && !full) {
          setMessages((prev) =>
            prev.map((m) => (m.id === asstId ? { ...m, text: "" } : m))
          );
        }
        resolveDone?.();
      });

      const unlistenError = await listen<{ requestId: string; error: string }>("ai-stream-error", (e) => {
        if (e.payload?.requestId !== requestId) return;
        rejectDone?.(new Error(String(e.payload?.error ?? "Unknown error")));
      });

      await aiCodeGenStream(requestId, payload);
      await donePromise;

      unlistenDelta();
      unlistenTool();
      unlistenDone();
      unlistenError();

      setAiResponse(full);
      setLastToolCallsUsed(toolCallsUsed);

      const patch = parsePatchFromResponse(full, agentMode, activeFilePath);
      setPendingPatch(patch);

      const newUsed = used + est + estimateTokens(full);
      setDailyUsedStorage(newUsed);
      setDailyTokenUsed(newUsed);
      log("AI response received");
    } catch (e) {
      log("AI error: " + String(e));
      setAiResponse("Error: " + String(e));
      setMessages((prev) => [...prev, { id: Date.now(), role: "assistant", text: "Error: " + String(e) }]);
    } finally {
      setAiLoading(false);
    }
  }, [aiPrompt, contextBlocks, log, agentMode, model, activeFilePath, projectDir, streamingCancelRef]);

  const tokenEstimate = estimateTokens(aiPrompt + (contextBlocks.length ? "\n" + contextBlocks.map((b) => b.content).join("\n") : ""));
  const isOllama = model.startsWith("ollama/");
  const isGrok = model.startsWith("grok/");
  const keyOk = isOllama || (isGrok ? grokApiKeySaved : apiKeySaved);
  const sendDisabled =
    aiLoading ||
    (!keyOk || (!isOllama && dailyTokenUsed + estimateTokens(aiPrompt.trim()) > DAILY_TOKEN_LIMIT));

  const resetDailyUsage = useCallback(() => {
    setDailyUsedStorage(0);
    setDailyTokenUsed(0);
  }, []);

  const updateModel = useCallback((next: string) => {
    setModel(next);
    try {
      localStorage.setItem("modai-ai-model", next);
    } catch {
      // ignore
    }
  }, []);

  const clearPendingPatch = useCallback(() => setPendingPatch(null), []);

  const runIteration = useCallback(async (quick: boolean) => {
    if (!iterationDiff) return null;
    const result = await runSelfIterate(iterationDiff, quick);
    setIterationRunResult(result);
    setMessages((prev) => [...prev, { id: Date.now(), role: "assistant", text: result.message }]);
    if (!quick || !result.success) {
      await saveIteration(aiPrompt.trim(), iterationDiff, result.success, result.message, null);
      await loadIterationHistory();
    }
    return result;
  }, [aiPrompt, iterationDiff, loadIterationHistory]);

  const adoptIteration = useCallback(async () => {
    if (!iterationDiff) return;
    await applyPatchToWorkspace(iterationDiff);
    setIterationDiff(null);
    setMessages((prev) => [...prev, { id: Date.now(), role: "assistant", text: "Patch adopted to workspace." }]);
  }, [iterationDiff]);

  const commitIteration = useCallback(async (message?: string) => {
    await commitIterationPatch(message?.trim() || "Self-iteration patch");
    setMessages((prev) => [...prev, { id: Date.now(), role: "assistant", text: "Patch committed." }]);
    await loadIterationHistory();
  }, [loadIterationHistory]);

  const reuseIteration = useCallback(async (record: IterationRecord) => {
    const loaded = record.diff ? record : await getIteration(record.id);
    if (!loaded?.diff) return;
    setAiPrompt(loaded.target || "");
    setIterationDiff(loaded.diff);
    const assistantText = buildIterateAssistantMessage(loaded.target || "history patch", loaded.diff);
    setAiResponse(assistantText);
    setMessages((prev) => [...prev, { id: Date.now(), role: "assistant", text: assistantText }]);
  }, []);

  const mode: AiMode = agentMode === "explain" ? "chat" : "code";
  const setMode = useCallback((m: AiMode) => {
    setAgentMode(m === "chat" ? "explain" : "edit-selection");
  }, []);

  const newChat = useCallback(() => {
    if (messages.length > 0) {
      saveCurrentSession(messages as { id: number; role: "user" | "assistant"; text: string }[]);
    }
    setMessages([]);
    setAiPrompt("");
    setPendingPatch(null);
    setAiResponse(null);
    setLastToolCallsUsed(null);
  }, [messages, saveCurrentSession, setAiPrompt]);

  const loadSessionById = useCallback(
    (id: string) => {
      const s = loadSession(id);
      if (!s) return;
      setMessages(s.messages as AiMessage[]);
      setAiPrompt("");
      setPendingPatch(null);
      const lastAsst = [...s.messages].reverse().find((m) => m.role === "assistant");
      setAiResponse(lastAsst?.text ?? null);
      setLastToolCallsUsed(null);
    },
    [loadSession]
  );

  return {
    projectDir,
    setProjectDir,
    sessions,
    newChat,
    loadSessionById,
    deleteSession,
    lastToolCallsUsed,
    apiKey,
    setApiKey,
    apiKeySaved,
    aiPrompt,
    setAiPrompt,
    aiLoading,
    aiResponse,
    dailyTokenUsed,
    dailyTokenLimit: DAILY_TOKEN_LIMIT,
    tokenEstimate,
    sendDisabled,
    saveApiKey,
    send,
    mode,
    setMode,
    model,
    setModel: updateModel,
    contextBlocks,
    setContextBlocks,
    resetDailyUsage,
    agentMode,
    setAgentMode,
    messages,
    pendingPatch,
    clearPendingPatch,
    activeFilePath,
    setActiveFilePath,
    iterationDiff,
    iterationRunResult,
    iterationHistory,
    loadIterationHistory,
    runIteration,
    adoptIteration,
    commitIteration,
    reuseIteration,
  };
}

export function useModelicaAI(log: (msg: string) => void) {
  return createAiHook(log, "modelica");
}

export function useJitAI(log: (msg: string) => void) {
  return createAiHook(log, "jit");
}
