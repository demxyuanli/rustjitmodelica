import { useState, useCallback, useEffect } from "react";
import type { ChatMessage } from "../components/ai/ai-markdown";

const STORAGE_PREFIX = "modai-ai-sessions";
const MAX_SESSIONS_PER_BUCKET = 50;

export interface AISession {
  id: string;
  title: string;
  messages: ChatMessage[];
  projectDir?: string | null;
  createdAt: number;
}

function storageKey(projectDir: string | null): string {
  return projectDir ? `${STORAGE_PREFIX}-${projectDir}` : `${STORAGE_PREFIX}-global`;
}

function loadSessionsFromStorage(projectDir: string | null): AISession[] {
  try {
    const raw = localStorage.getItem(storageKey(projectDir));
    if (!raw) return [];
    const parsed = JSON.parse(raw) as AISession[];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function saveSessionsToStorage(projectDir: string | null, sessions: AISession[]): void {
  try {
    const trimmed = sessions.slice(-MAX_SESSIONS_PER_BUCKET);
    localStorage.setItem(storageKey(projectDir), JSON.stringify(trimmed));
  } catch {
    // ignore
  }
}

function makeTitle(messages: ChatMessage[]): string {
  const firstUser = messages.find((m) => m.role === "user");
  const text = firstUser?.text?.trim() ?? "";
  if (!text) return "Session";
  const line = text.split("\n")[0] ?? "";
  return line.length > 40 ? line.slice(0, 40) + "..." : line;
}

export function useAISessions(projectDir: string | null) {
  const [sessions, setSessions] = useState<AISession[]>(() =>
    loadSessionsFromStorage(projectDir)
  );

  useEffect(() => {
    setSessions(loadSessionsFromStorage(projectDir));
  }, [projectDir]);

  const saveCurrentSession = useCallback(
    (messages: ChatMessage[]) => {
      if (messages.length === 0) return null;
      const id = `s-${Date.now()}`;
      const session: AISession = {
        id,
        title: makeTitle(messages),
        messages: [...messages],
        projectDir: projectDir ?? null,
        createdAt: Date.now(),
      };
      setSessions((prev) => {
        const next = [...prev, session].slice(-MAX_SESSIONS_PER_BUCKET);
        saveSessionsToStorage(projectDir, next);
        return next;
      });
      return id;
    },
    [projectDir]
  );

  const loadSession = useCallback((id: string): AISession | null => {
    const all = loadSessionsFromStorage(projectDir);
    return all.find((s) => s.id === id) ?? null;
  }, [projectDir]);

  const deleteSession = useCallback(
    (id: string) => {
      setSessions((prev) => {
        const next = prev.filter((s) => s.id !== id);
        saveSessionsToStorage(projectDir, next);
        return next;
      });
    },
    [projectDir]
  );

  return {
    sessions,
    saveCurrentSession,
    loadSession,
    deleteSession,
  };
}
