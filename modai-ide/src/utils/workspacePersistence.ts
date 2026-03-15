/**
 * Workspace state persistence: meta in localStorage, dirty content drafts in IndexedDB.
 * Keys are derived from project path (normalized + hashed) for stability.
 */

const META_PREFIX = "modai-workspace-meta-";
const DB_NAME = "modai-workspace-drafts";
const DB_VERSION = 1;
const STORE_NAME = "drafts";

export interface EditorTabSerial {
  id: string;
  path: string;
  dirty: boolean;
  projectPath?: string | null;
  readOnly?: boolean;
  modelName?: string;
}

export interface EditorGroupStateSerial {
  tabs: EditorTabSerial[];
  activeIndex: number;
}

export interface CursorPosition {
  lineNumber: number;
  column: number;
}

export interface WorkspaceMetaSerial {
  version: number;
  editorGroups: EditorGroupStateSerial[];
  focusedGroupIndex: number;
  splitRatio: number;
  cursorByPath?: Record<string, CursorPosition>;
}

function normalizePath(p: string): string {
  return p.replace(/\\/g, "/").trim();
}

function simpleHash(s: string): string {
  let h = 0;
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    h = (h << 5) - h + c;
    h |= 0;
  }
  return Math.abs(h).toString(16);
}

export function getWorkspaceStateKey(projectDir: string): string {
  const normalized = normalizePath(projectDir);
  return normalized ? simpleHash(normalized) : "";
}

function getMetaKey(projectKey: string): string {
  return META_PREFIX + projectKey;
}

export function loadWorkspaceMeta(projectKey: string): WorkspaceMetaSerial | null {
  if (typeof localStorage === "undefined" || !projectKey) return null;
  try {
    const raw = localStorage.getItem(getMetaKey(projectKey));
    if (!raw) return null;
    const parsed = JSON.parse(raw) as WorkspaceMetaSerial;
    if (parsed?.version !== 1 || !Array.isArray(parsed.editorGroups)) return null;
    return parsed;
  } catch {
    return null;
  }
}

export function saveWorkspaceMeta(projectKey: string, meta: WorkspaceMetaSerial): void {
  if (typeof localStorage === "undefined" || !projectKey) return;
  try {
    localStorage.setItem(getMetaKey(projectKey), JSON.stringify(meta));
  } catch {
    /* ignore */
  }
}

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onerror = () => reject(req.error);
    req.onsuccess = () => resolve(req.result);
    req.onupgradeneeded = () => {
      if (!req.result.objectStoreNames.contains(STORE_NAME)) {
        req.result.createObjectStore(STORE_NAME);
      }
    };
  });
}

export async function loadWorkspaceDrafts(projectKey: string): Promise<Record<string, string>> {
  if (!projectKey) return {};
  try {
    const db = await openDb();
    return new Promise((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, "readonly");
      const store = tx.objectStore(STORE_NAME);
      const req = store.get(projectKey);
      req.onerror = () => {
        db.close();
        reject(req.error);
      };
      req.onsuccess = () => {
        db.close();
        const value = req.result;
        resolve(value && typeof value === "object" && !Array.isArray(value) ? value : {});
      };
    });
  } catch {
    return {};
  }
}

export async function saveWorkspaceDrafts(
  projectKey: string,
  drafts: Record<string, string>
): Promise<void> {
  if (!projectKey) return;
  try {
    const db = await openDb();
    return new Promise((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, "readwrite");
      const store = tx.objectStore(STORE_NAME);
      store.put(drafts, projectKey);
      tx.oncomplete = () => {
        db.close();
        resolve();
      };
      tx.onerror = () => {
        db.close();
        reject(tx.error);
      };
    });
  } catch {
    /* ignore */
  }
}

const JIT_META_PREFIX = "modai-jit-workspace-meta-";

export interface JitOpenFileTabSerial {
  path: string;
  type: "rust" | "modelica";
  dirty: boolean;
}

export interface JitWorkspaceMetaSerial {
  version: number;
  openFiles: JitOpenFileTabSerial[];
  activeFilePath: string | null;
}

export function loadJitWorkspaceMeta(repoKey: string): JitWorkspaceMetaSerial | null {
  if (typeof localStorage === "undefined" || !repoKey) return null;
  try {
    const raw = localStorage.getItem(JIT_META_PREFIX + repoKey);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as JitWorkspaceMetaSerial;
    if (parsed?.version !== 1 || !Array.isArray(parsed.openFiles)) return null;
    return parsed;
  } catch {
    return null;
  }
}

export function saveJitWorkspaceMeta(repoKey: string, meta: JitWorkspaceMetaSerial): void {
  if (typeof localStorage === "undefined" || !repoKey) return;
  try {
    localStorage.setItem(JIT_META_PREFIX + repoKey, JSON.stringify(meta));
  } catch {
    /* ignore */
  }
}

export function getJitWorkspaceDraftsKey(repoRoot: string): string {
  return "jit-" + getWorkspaceStateKey(repoRoot);
}
