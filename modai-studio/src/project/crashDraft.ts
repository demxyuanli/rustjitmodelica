import type { DiagramDocument, StudioProjectManifest } from "../api/types";
import { t, tf } from "../i18n";

const DRAFT_KEY = "modai-studio.crash-draft";
const HEARTBEAT_KEY = "modai-studio.session-alive";
const STALE_MS = 15_000;

export type CrashDraft = {
  rootDir: string | null;
  modelName: string;
  savedAt: number;
  manifest: StudioProjectManifest;
  diagram: DiagramDocument;
  subsystems: Record<string, DiagramDocument>;
};

export function writeCrashDraft(draft: CrashDraft): void {
  try {
    localStorage.setItem(DRAFT_KEY, JSON.stringify(draft));
  } catch {
    /* quota / private mode */
  }
}

export function readCrashDraft(): CrashDraft | null {
  try {
    const raw = localStorage.getItem(DRAFT_KEY);
    if (!raw) return null;
    const d = JSON.parse(raw) as CrashDraft;
    if (!d || typeof d.savedAt !== "number" || !d.diagram) return null;
    return d;
  } catch {
    return null;
  }
}

export function clearCrashDraft(): void {
  try {
    localStorage.removeItem(DRAFT_KEY);
  } catch {
    /* ignore */
  }
}

export function draftHasWork(d: CrashDraft): boolean {
  if (d.diagram.nodes.length > 0 || d.diagram.edges.length > 0) return true;
  return Object.values(d.subsystems ?? {}).some((s) => s.nodes.length > 0 || s.edges.length > 0);
}

export function touchSessionHeartbeat(): void {
  try {
    localStorage.setItem(HEARTBEAT_KEY, String(Date.now()));
  } catch {
    /* ignore */
  }
}

export function clearSessionHeartbeat(): void {
  try {
    localStorage.removeItem(HEARTBEAT_KEY);
  } catch {
    /* ignore */
  }
}

export function isSessionStale(): boolean {
  try {
    const raw = localStorage.getItem(HEARTBEAT_KEY);
    if (!raw) return true;
    const ts = Number(raw);
    if (!Number.isFinite(ts)) return true;
    return Date.now() - ts > STALE_MS;
  } catch {
    return true;
  }
}

export function confirmRestoreDraft(draft: CrashDraft): boolean {
  const when = new Date(draft.savedAt).toLocaleString();
  return window.confirm(tf("restoreCrashDraft", { name: draft.modelName || t("unsaved"), time: when }));
}
