import { useEffect, useRef } from "react";
import { indexRepoRoot, getAppSettings, getAppDataRoot } from "../api/tauri";
import type { AppSettings } from "../api/tauri";
import { PREFS_KEYS, readPref, writePref } from "../utils/prefsConstants";

let lastProjectRestoreAttempted = false;

function scheduleRestoreLastProjectOnce(
  setWorkspaceMode: (mode: "modelica" | "component-library" | "compiler-iterate" | "regression") => void,
  setProjectDirFromPath: (path: string) => Promise<void>,
) {
  requestAnimationFrame(() => {
    setTimeout(() => {
      if (lastProjectRestoreAttempted) return;
      const restoreLayout = readPref(PREFS_KEYS.restoreLayout, (s) => s === "true", true);
      if (!restoreLayout) return;
      const lastDir = readPref(PREFS_KEYS.lastProjectDir, (s) => (s && s.trim() ? s.trim() : ""), "");
      if (!lastDir) return;
      const attempt = (retry = false) => {
        lastProjectRestoreAttempted = true;
        setProjectDirFromPath(lastDir)
          .then(() => setWorkspaceMode("modelica"))
          .catch(() => {
            if (retry) {
              writePref(PREFS_KEYS.lastProjectDir, "");
              return;
            }
            lastProjectRestoreAttempted = false;
            setTimeout(() => attempt(true), 500);
          });
      };
      attempt(false);
    }, 200);
  });
}

export function useAppBootstrapEffects(options: {
  setRepoRoot: (path: string) => void;
  setAppSettingsState: (s: AppSettings) => void;
  setAppDataRoot: (path: string) => void;
  setWorkspaceMode: (mode: "modelica" | "component-library" | "compiler-iterate" | "regression") => void;
  setProjectDirFromPath: (path: string) => Promise<void>;
}): void {
  const {
    setRepoRoot,
    setAppSettingsState,
    setAppDataRoot,
    setWorkspaceMode,
    setProjectDirFromPath,
  } = options;

  const indexRepoRootDoneRef = useRef(false);
  useEffect(() => {
    if (indexRepoRootDoneRef.current) return;
    const start = performance.now?.() ?? Date.now();
    const run = () => {
      indexRepoRootDoneRef.current = true;
      indexRepoRoot()
        .then((r) => setRepoRoot(r))
        .catch(() => {})
        .finally(() => {
          const end = performance.now?.() ?? Date.now();
          // eslint-disable-next-line no-console
          console.log("[modai-prof] indexRepoRoot took", end - start, "ms");
        });
    };
    const t = window.setTimeout(run, 0);
    return () => window.clearTimeout(t);
  }, [setRepoRoot]);

  useEffect(() => {
    const handler = (event: MouseEvent) => {
      const target = event.target as HTMLElement | null;
      if (!target) return;
      const tag = target.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA") {
        return;
      }
      if (target.isContentEditable) {
        return;
      }
      let el: HTMLElement | null = target;
      while (el) {
        if (el.dataset && el.dataset.allowBrowserContextmenu === "true") {
          return;
        }
        el = el.parentElement;
      }
      event.preventDefault();
    };
    window.addEventListener("contextmenu", handler);
    return () => {
      window.removeEventListener("contextmenu", handler);
    };
  }, []);

  useEffect(() => {
    getAppSettings().then(setAppSettingsState).catch(() => {});
    getAppDataRoot().then(setAppDataRoot).catch(() => {});
  }, [setAppSettingsState, setAppDataRoot]);

  useEffect(() => {
    const start = performance.now?.() ?? Date.now();
    scheduleRestoreLastProjectOnce(setWorkspaceMode, setProjectDirFromPath);
    const end = performance.now?.() ?? Date.now();
    // eslint-disable-next-line no-console
    console.log("[modai-prof] scheduleRestoreLastProjectOnce took", end - start, "ms");
  }, [setWorkspaceMode, setProjectDirFromPath]);
}
