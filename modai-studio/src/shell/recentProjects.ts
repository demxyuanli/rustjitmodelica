import { t } from "../i18n";

const KEY = "modai-studio.recentProjects";
const MAX = 8;

export type RecentProject = {
  path: string;
  modelName: string;
  openedAt: number;
};

function normalizePath(path: string): string {
  let s = path.replace(/\\/g, "/").trim();
  if (s.startsWith("//?/")) s = s.slice(4);
  return s.replace(/\/+$/, "");
}

function pathKey(path: string): string {
  return normalizePath(path).toLowerCase();
}

export function recentProjectFolder(path: string): string {
  const parts = normalizePath(path).split("/").filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1]! : path;
}

function parseList(raw: unknown): RecentProject[] {
  if (!Array.isArray(raw)) return [];
  const seen = new Set<string>();
  const out: RecentProject[] = [];
  for (const item of raw) {
    if (!item || typeof item !== "object") continue;
    const rec = item as Partial<RecentProject>;
    if (typeof rec.path !== "string" || !rec.path.trim()) continue;
    const path = normalizePath(rec.path);
    const key = pathKey(path);
    if (seen.has(key)) continue;
    seen.add(key);
    out.push({
      path,
      modelName: typeof rec.modelName === "string" ? rec.modelName : recentProjectFolder(path),
      openedAt: typeof rec.openedAt === "number" ? rec.openedAt : 0,
    });
    if (out.length >= MAX) break;
  }
  return out;
}

export function loadRecentProjects(): RecentProject[] {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return [];
    return parseList(JSON.parse(raw) as unknown);
  } catch {
    return [];
  }
}

function writeList(next: RecentProject[]): RecentProject[] {
  try {
    localStorage.setItem(KEY, JSON.stringify(next));
  } catch {
    /* ignore quota */
  }
  return next;
}

export function rememberRecentProject(path: string, modelName: string): RecentProject[] {
  const norm = normalizePath(path);
  if (!norm) return loadRecentProjects();
  const key = pathKey(norm);
  const label = modelName.trim() || recentProjectFolder(norm);
  const next = [
    { path: norm, modelName: label, openedAt: Date.now() },
    ...loadRecentProjects().filter((p) => pathKey(p.path) !== key),
  ].slice(0, MAX);
  return writeList(next);
}

export function forgetRecentProject(path: string): RecentProject[] {
  const key = pathKey(path);
  return writeList(loadRecentProjects().filter((p) => pathKey(p.path) !== key));
}

export function confirmDiscardUnsaved(dirty: boolean): boolean {
  if (!dirty) return true;
  return window.confirm(t("unsavedChanges"));
}
