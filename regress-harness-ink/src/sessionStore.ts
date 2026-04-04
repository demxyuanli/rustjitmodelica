import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import type { SessionContext } from "./replDispatch.js";
import { defaultSessionContext } from "./replDispatch.js";

const SESSION_FILE = ".regress-harness-ink-session.json";

/**
 * Walk up from startDir; first directory containing `.git` is the workspace root.
 * If none found, use startDir resolved (session file lives next to cwd).
 */
export function resolveWorkspaceRoot(startDir: string): string {
  let dir = resolve(startDir);
  for (let depth = 0; depth < 32; depth++) {
    if (existsSync(join(dir, ".git"))) {
      return dir;
    }
    const parent = dirname(dir);
    if (parent === dir) {
      break;
    }
    dir = parent;
  }
  return resolve(startDir);
}

export function getSessionFilePath(workspaceRoot: string): string {
  return join(workspaceRoot, SESSION_FILE);
}

export function mergeSessionContext(
  partial: Partial<SessionContext> | null | undefined
): SessionContext {
  const b = defaultSessionContext();
  if (!partial || typeof partial !== "object") {
    return b;
  }
  return {
    cmdPrefix: Array.isArray(partial.cmdPrefix)
      ? [...partial.cmdPrefix]
      : [...b.cmdPrefix],
    repoRoot: partial.repoRoot ?? b.repoRoot,
    rustmodlicaExe: partial.rustmodlicaExe ?? b.rustmodlicaExe,
    cargoTargetDir: partial.cargoTargetDir ?? b.cargoTargetDir,
    config: partial.config ?? b.config,
    tier: partial.tier ?? b.tier,
    tags: partial.tags !== undefined ? partial.tags : b.tags,
    workers: partial.workers ?? b.workers,
    baseline: partial.baseline ?? b.baseline,
    incremental: partial.incremental ?? b.incremental,
    manifest: partial.manifest ?? b.manifest,
    dataRoot: partial.dataRoot ?? b.dataRoot,
    outDir: partial.outDir ?? b.outDir,
    format:
      partial.format === "json" || partial.format === "human"
        ? partial.format
        : b.format,
    ndjson: typeof partial.ndjson === "boolean" ? partial.ndjson : b.ndjson,
    summaryCompat:
      typeof partial.summaryCompat === "boolean"
        ? partial.summaryCompat
        : b.summaryCompat,
    progress:
      typeof partial.progress === "boolean" ? partial.progress : b.progress,
  };
}

type StoreV1 = { version: 1; context: SessionContext };

export function loadSessionFromDisk(workspaceRoot: string): SessionContext {
  const p = getSessionFilePath(workspaceRoot);
  if (!existsSync(p)) {
    return defaultSessionContext();
  }
  try {
    const raw = readFileSync(p, "utf8");
    const j = JSON.parse(raw) as StoreV1;
    if (!j || j.version !== 1 || !j.context || typeof j.context !== "object") {
      return defaultSessionContext();
    }
    return mergeSessionContext(j.context);
  } catch {
    return defaultSessionContext();
  }
}

export function saveSessionToDisk(
  workspaceRoot: string,
  ctx: SessionContext
): void {
  const p = getSessionFilePath(workspaceRoot);
  const payload: StoreV1 = { version: 1, context: ctx };
  writeFileSync(p, JSON.stringify(payload, null, 2), "utf8");
}
