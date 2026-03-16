import { useState, useCallback } from "react";
import { PREFS_KEYS, readPref, writePref } from "../utils/prefsConstants";

const MAX_RECENT = 10;

function normalizePath(path: string): string {
  let s = path.replace(/\\/g, "/").trim();
  if (s.startsWith("//?/")) s = s.slice(4);
  return s;
}

function pathDedupKey(path: string): string {
  return normalizePath(path).toLowerCase();
}

function dedupeRecentList(paths: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const p of paths) {
    const n = normalizePath(p);
    if (!n) continue;
    const key = pathDedupKey(n);
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(n);
  }
  return out;
}

function readRecentFromStorage(): string[] {
  const raw = readPref(
    PREFS_KEYS.recentProjectDirs,
    (s) => {
      try {
        const a = JSON.parse(s ?? "[]");
        return Array.isArray(a) ? a.filter((x): x is string => typeof x === "string") : [];
      } catch {
        return [];
      }
    },
    [] as string[]
  );
  return dedupeRecentList(raw).slice(0, MAX_RECENT);
}

export function useRecentProjects() {
  const [recentProjects, setRecentProjects] = useState<string[]>(readRecentFromStorage);

  const addRecentProject = useCallback((path: string) => {
    const norm = normalizePath(path);
    if (!norm) return;
    const key = pathDedupKey(norm);
    setRecentProjects((prev) => {
      const next = dedupeRecentList([norm, ...prev.filter((p) => pathDedupKey(normalizePath(p)) !== key)]).slice(0, MAX_RECENT);
      writePref(PREFS_KEYS.recentProjectDirs, JSON.stringify(next));
      return next;
    });
  }, []);

  return { recentProjects, addRecentProject };
}

export function recentProjectDisplayName(path: string): string {
  const norm = normalizePath(path);
  const parts = norm.split("/").filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1]! : norm || path;
}
