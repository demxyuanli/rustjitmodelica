import { useCallback, useEffect, useState, type RefObject } from "react";
import type { EditorWorkbenchRef } from "../components/EditorWorkbench";
import type { EditorTab } from "../components/EditorTabBar";
import {
  getWorkspaceStateKey,
  loadWorkspaceDrafts,
  loadWorkspaceMeta,
  saveWorkspaceDrafts,
  saveWorkspaceMeta,
  type WorkspaceMetaSerial,
} from "../utils/workspacePersistence";

export type RestoredModelicaWorkspace = {
  projectDir: string;
  meta: WorkspaceMetaSerial;
  drafts: Record<string, string>;
};

export function useModelicaWorkspacePersistence(
  projectDir: string | null,
  workbenchRef: RefObject<EditorWorkbenchRef | null>,
): RestoredModelicaWorkspace | null {
  const [restoredWorkspace, setRestoredWorkspace] = useState<RestoredModelicaWorkspace | null>(null);

  useEffect(() => {
    if (!projectDir) {
      setRestoredWorkspace(null);
      return;
    }
    const projectKey = getWorkspaceStateKey(projectDir);
    if (!projectKey) return;
    const start = performance.now?.() ?? Date.now();
    const meta = loadWorkspaceMeta(projectKey);
    loadWorkspaceDrafts(projectKey).then((drafts) => {
      const end = performance.now?.() ?? Date.now();
      // eslint-disable-next-line no-console
      console.log("[modai-prof] restore modelica workspace state took", end - start, "ms");
      if (meta) {
        setRestoredWorkspace({ projectDir, meta, drafts });
      } else {
        setRestoredWorkspace(null);
      }
    });
  }, [projectDir]);

  const persistWorkspaceState = useCallback(() => {
    const dir = projectDir;
    if (!dir) return;
    const snapshot = workbenchRef.current?.getWorkspaceState?.();
    if (!snapshot) return;
    const hasTabs = snapshot.editorGroups.some((g) => g.tabs.length > 0);
    if (!hasTabs) return;
    const projectKey = getWorkspaceStateKey(dir);
    if (!projectKey) return;
    const meta: WorkspaceMetaSerial = {
      version: 1,
      editorGroups: snapshot.editorGroups.map((g) => ({
        tabs: g.tabs.map((t: EditorTab) => ({
          id: t.id,
          path: t.path,
          dirty: t.dirty,
          projectPath: t.projectPath,
          readOnly: t.readOnly,
          modelName: t.modelName,
        })),
        activeIndex: g.activeIndex,
      })),
      focusedGroupIndex: snapshot.focusedGroupIndex,
      splitRatio: snapshot.splitRatio,
    };
    saveWorkspaceMeta(projectKey, meta);
    const drafts: Record<string, string> = {};
    const norm = (p: string) => p.replace(/\\/g, "/");
    for (const g of snapshot.editorGroups) {
      for (const tab of g.tabs) {
        if (tab.dirty) {
          const key = tab.projectPath != null ? norm(tab.projectPath) : tab.id;
          const content = snapshot.contentByPath[key];
          if (content !== undefined) drafts[key] = content;
        }
      }
    }
    saveWorkspaceDrafts(projectKey, drafts);
  }, [projectDir, workbenchRef]);

  useEffect(() => {
    if (!projectDir) return;
    const id = setInterval(persistWorkspaceState, 2000);
    return () => clearInterval(id);
  }, [projectDir, persistWorkspaceState]);

  useEffect(() => {
    const onUnload = () => persistWorkspaceState();
    window.addEventListener("beforeunload", onUnload);
    window.addEventListener("pagehide", onUnload);
    return () => {
      window.removeEventListener("beforeunload", onUnload);
      window.removeEventListener("pagehide", onUnload);
    };
  }, [persistWorkspaceState]);

  return restoredWorkspace;
}
