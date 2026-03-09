import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

const DAILY_TOKEN_LIMIT = 50000;

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

export function useAI(log: (msg: string) => void) {
  const [apiKey, setApiKey] = useState("");
  const [apiKeySaved, setApiKeySaved] = useState(false);
  const [aiPrompt, setAiPrompt] = useState("");
  const [aiLoading, setAiLoading] = useState(false);
  const [aiResponse, setAiResponse] = useState<string | null>(null);
  const [dailyTokenUsed, setDailyTokenUsed] = useState(getDailyUsed);

  useEffect(() => {
    invoke("get_api_key")
      .then((k) => {
        setApiKey((k as string) ? "********" : "");
        setApiKeySaved(!!(k as string));
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    setDailyTokenUsed(getDailyUsed());
  }, []);

  const saveApiKey = useCallback(async (key: string) => {
    if (!key || key === "********") return;
    try {
      await invoke("set_api_key", { apiKey: key });
      setApiKeySaved(true);
      setApiKey("********");
      log("API key saved");
    } catch (e) {
      log("API key save error: " + String(e));
    }
  }, [log]);

  const send = useCallback(async () => {
    if (!aiPrompt.trim()) return;
    const est = estimateTokens(aiPrompt.trim());
    const used = getDailyUsed();
    if (used + est > DAILY_TOKEN_LIMIT) {
      log("Daily token limit reached. Used: " + used + ", limit: " + DAILY_TOKEN_LIMIT);
      return;
    }
    setAiLoading(true);
    setAiResponse(null);
    try {
      const result = (await invoke("ai_code_gen", { prompt: aiPrompt.trim() })) as string;
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
  }, [aiPrompt, log]);

  const tokenEstimate = estimateTokens(aiPrompt);
  const sendDisabled = aiLoading || !apiKeySaved || dailyTokenUsed + estimateTokens(aiPrompt.trim()) > DAILY_TOKEN_LIMIT;

  return {
    apiKey, setApiKey,
    apiKeySaved,
    aiPrompt, setAiPrompt,
    aiLoading,
    aiResponse,
    dailyTokenUsed,
    dailyTokenLimit: DAILY_TOKEN_LIMIT,
    tokenEstimate,
    sendDisabled,
    saveApiKey,
    send,
  };
}
