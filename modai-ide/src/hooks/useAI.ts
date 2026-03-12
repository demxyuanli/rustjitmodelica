import { useState, useCallback, useEffect } from "react";
import { getApiKey, setApiKey as setApiKeyCommand, aiCodeGen } from "../api/tauri";

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

type AiMode = "chat" | "code";

function createAiHook(log: (msg: string) => void, kind: "modelica" | "jit") {
  const [apiKey, setApiKey] = useState("");
  const [apiKeySaved, setApiKeySaved] = useState(false);
  const [aiPrompt, setAiPrompt] = useState("");
  const [aiLoading, setAiLoading] = useState(false);
  const [aiResponse, setAiResponse] = useState<string | null>(null);
  const [dailyTokenUsed, setDailyTokenUsed] = useState(getDailyUsed);
  const [mode, setMode] = useState<AiMode>("code");
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

  useEffect(() => {
    getApiKey()
      .then((k) => {
        setApiKey(k ? "********" : "");
        setApiKeySaved(!!k);
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    setDailyTokenUsed(getDailyUsed());
  }, []);

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

  const send = useCallback(async () => {
    if (!aiPrompt.trim()) return;
    const basePrompt = aiPrompt.trim();
    const contextText = contextBlocks.map((b) => b.content).join("\n\n");
    const est = estimateTokens(basePrompt + "\n" + contextText);
    const used = getDailyUsed();
    if (used + est > DAILY_TOKEN_LIMIT) {
      log("Daily token limit reached. Used: " + used + ", limit: " + DAILY_TOKEN_LIMIT);
      return;
    }
    setAiLoading(true);
    setAiResponse(null);
    try {
      const system =
        mode === "code"
          ? kind === "jit"
            ? "You are an expert Rust JIT compiler and test suite coding assistant. Reply with clear, directly usable code or concrete editing instructions."
            : "You are an expert Modelica and Rust IDE coding assistant. Reply with clear, directly usable code or concrete editing instructions."
          : undefined;

      const payload = {
        prompt: basePrompt,
        system,
        contextBlocks: contextBlocks.length > 0 ? contextBlocks : undefined,
        options: {
          model: model || DEFAULT_MODEL,
        },
      };

      const result = await aiCodeGen(payload);
      setAiResponse(result);
      const newUsed = used + est + estimateTokens(result);
      setDailyUsedStorage(newUsed);
      setDailyTokenUsed(newUsed);
      log("AI response received");
    } catch (e) {
      log("AI error: " + String(e));
      setAiResponse("Error: " + String(e));
    } finally {
      setAiLoading(false);
    }
  }, [aiPrompt, contextBlocks, log, mode, model]);

  const tokenEstimate = estimateTokens(aiPrompt + (contextBlocks.length ? "\n" + contextBlocks.map((b) => b.content).join("\n") : ""));
  const sendDisabled = aiLoading || !apiKeySaved || dailyTokenUsed + estimateTokens(aiPrompt.trim()) > DAILY_TOKEN_LIMIT;

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

  return {
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
  };
}

export function useModelicaAI(log: (msg: string) => void) {
  return createAiHook(log, "modelica");
}

export function useJitAI(log: (msg: string) => void) {
  return createAiHook(log, "jit");
}
