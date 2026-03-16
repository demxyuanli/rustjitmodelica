import { useState, useCallback, useRef, useEffect, lazy, Suspense } from "react";
import type monaco from "monaco-editor";
import { listen } from "@tauri-apps/api/event";
import { setLang } from "./i18n";

import { useLayout } from "./hooks/useLayout";
import { useProject, pathToModelName } from "./hooks/useProject";
import { useRecentProjects } from "./hooks/useRecentProjects";
import { useDiagramScheme } from "./contexts/DiagramSchemeContext";
import { useSimulation } from "./hooks/useSimulation";
import { useModelicaAI, type AiContextBlock } from "./hooks/useAI";

import { Titlebar } from "./components/Titlebar";
import { FileTree } from "./components/FileTree";
import { OutlineSection } from "./components/OutlineSection";
import { TimelineSection } from "./components/TimelineSection";
import { SourceControlView } from "./components/SourceControlView";
import { EditorWorkbench, type EditorWorkbenchRef } from "./components/EditorWorkbench";
import { WelcomeView } from "./components/WelcomeView";
import { ComponentLibraryWorkspace } from "./components/ComponentLibraryWorkspace";
import { StatusBar, type IndexStatusInfo } from "./components/StatusBar";
import { AIPanel } from "./components/AIPanel";
import { SearchPanel } from "./components/SearchPanel";
import { AppIcon } from "./components/Icon";
import { IconButton } from "./components/IconButton";
import { emit } from "@tauri-apps/api/event";

const DiffView = lazy(() => import("./components/DiffView").then((m) => ({ default: m.DiffView })));
const GitGraphView = lazy(() => import("./components/GitGraphView").then((m) => ({ default: m.GitGraphView })));
const SimulationPanel = lazy(() => import("./components/SimulationPanel").then((m) => ({ default: m.SimulationPanel })));
import { NewModelDialog } from "./components/diagram/NewModelDialog";
import { JitIdeWorkspace } from "./components/JitIdeWorkspace";
import { GlobalSettingsPanel } from "./components/GlobalSettingsPanel";
import type { IndexActionState } from "./components/SettingsContent";
import { t } from "./i18n";
import type { JitCenterView } from "./hooks/useJitLayout";
import { DEFAULT_MODEL_BOUNCING_BALL } from "./examples";
import {
  indexRepoRoot,
  indexBuild,
  indexStats,
  indexStartWatcher,
  indexStopWatcher,
  indexBuildRepo,
  indexRefresh,
  indexRebuild,
  indexRefreshRepo,
  indexRebuildRepo,
  writeProjectFile,
  getAppSettings,
  setAppSettings,
  getAppDataRoot,
  applyPatchToProject,
  readProjectFile,
} from "./api/tauri";
import { parsePathsFromDiff } from "./components/ai/ai-markdown";
import type { AppSettings } from "./api/tauri";
import { PREFS_KEYS, readPref, writePref } from "./utils/prefsConstants";
import {
  getWorkspaceStateKey,
  loadWorkspaceMeta,
  loadWorkspaceDrafts,
  saveWorkspaceMeta,
  saveWorkspaceDrafts,
  type WorkspaceMetaSerial,
} from "./utils/workspacePersistence";
import type { EditorTab } from "./components/EditorTabBar";
import "./App.css";

type WorkbenchBottomTab = "problems" | "output" | "results" | "deps";

let lastProjectRestoreAttempted = false;

function scheduleRestoreLastProjectOnce(
  setWorkspaceMode: (mode: "modelica" | "component-library" | "compiler-iterate") => void,
  setProjectDirFromPath: (path: string) => Promise<void>
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

function App() {
  const [modelName, setModelName] = useState("BouncingBall");
  const [logLines, setLogLines] = useState<string[]>([]);
  const [contentByPath, setContentByPath] = useState<Record<string, string>>({});
  const [openFilePath, setOpenFilePath] = useState<string | null>(null);
  const [code, setCode] = useState(DEFAULT_MODEL_BOUNCING_BALL);
  const [cursorPosition, setCursorPosition] = useState<{ lineNumber: number; column: number } | null>(null);
  const [currentSelection, setCurrentSelection] = useState<{ path: string | null; text: string | null }>({ path: null, text: null });

  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<typeof monaco | null>(null);
  const workbenchRef = useRef<EditorWorkbenchRef>(null);

  const log = useCallback((msg: string) => {
    setLogLines((prev) => [...prev, `${new Date().toISOString().slice(11, 19)} ${msg}`]);
  }, []);

  const layout = useLayout();
  const project = useProject();
  const { recentProjects, addRecentProject } = useRecentProjects();
  const diagramScheme = useDiagramScheme();
  const sim = useSimulation(log);
  const ai = useModelicaAI(log);

  const [indexStatus, setIndexStatus] = useState<IndexStatusInfo | null>(null);
  const [repoRoot, setRepoRoot] = useState<string | null>(null);
  const [indexAction, setIndexAction] = useState<IndexActionState>({ running: false, action: null, done: 0, total: 0 });
  const [showJitViewMenu, setShowJitViewMenu] = useState(false);
  const [jitCenterViewRequest, setJitCenterViewRequest] = useState<JitCenterView | null | undefined>(undefined);
  const [jitActiveCenterView, setJitActiveCenterView] = useState<JitCenterView | null>(null);
  const [requestedSimulationTab, setRequestedSimulationTab] = useState<WorkbenchBottomTab | null>(null);
  const [focusedDiagramSymbol, setFocusedDiagramSymbol] = useState<string | null>(null);
  const [libraryRefreshToken, setLibraryRefreshToken] = useState(0);
  const [showNewModelDialog, setShowNewModelDialog] = useState(false);
  const [appSettings, setAppSettingsState] = useState<AppSettings | null>(null);
  const [appDataRoot, setAppDataRoot] = useState<string | null>(null);
  const [restoredWorkspace, setRestoredWorkspace] = useState<{
    projectDir: string;
    meta: WorkspaceMetaSerial;
    drafts: Record<string, string>;
  } | null>(null);

  useEffect(() => {
    indexRepoRoot().then((r) => setRepoRoot(r)).catch(() => {});
  }, []);

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
  }, []);

  useEffect(() => {
    scheduleRestoreLastProjectOnce(layout.setWorkspaceMode, project.setProjectDirFromPath);
  }, [layout.setWorkspaceMode, project.setProjectDirFromPath]);

  useEffect(() => {
    if (project.projectDir) addRecentProject(project.projectDir);
  }, [project.projectDir, addRecentProject]);

  useEffect(() => {
    ai.setProjectDir(project.projectDir ?? null);
  }, [ai.setProjectDir, project.projectDir]);

  useEffect(() => {
    if (!project.projectDir) {
      setRestoredWorkspace(null);
      return;
    }
    const projectKey = getWorkspaceStateKey(project.projectDir);
    if (!projectKey) return;
    const meta = loadWorkspaceMeta(projectKey);
    loadWorkspaceDrafts(projectKey).then((drafts) => {
      if (meta) {
        setRestoredWorkspace({ projectDir: project.projectDir!, meta, drafts });
      } else {
        setRestoredWorkspace(null);
      }
    });
  }, [project.projectDir]);

  const persistWorkspaceState = useCallback(() => {
    const dir = project.projectDir;
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
  }, [project.projectDir]);

  useEffect(() => {
    if (!project.projectDir) return;
    const id = setInterval(persistWorkspaceState, 2000);
    return () => clearInterval(id);
  }, [project.projectDir, persistWorkspaceState]);

  useEffect(() => {
    const onUnload = () => persistWorkspaceState();
    window.addEventListener("beforeunload", onUnload);
    window.addEventListener("pagehide", onUnload);
    return () => {
      window.removeEventListener("beforeunload", onUnload);
      window.removeEventListener("pagehide", onUnload);
    };
  }, [persistWorkspaceState]);

  useEffect(() => {
    setLang(layout.lang);
  }, [layout.lang]);

  useEffect(() => {
    if (layout.workspaceMode === "modelica") {
      if (!project.projectDir) {
        setIndexStatus(null);
        return;
      }
      const dir = project.projectDir;
      setIndexStatus({ fileCount: 0, symbolCount: 0, state: "building" });
      indexBuild(dir)
        .then((stats: any) => {
          setIndexStatus({
            fileCount: stats.fileCount ?? 0,
            symbolCount: stats.symbolCount ?? 0,
            state: "ready",
          });
        })
        .catch(() => setIndexStatus(null));

      indexStartWatcher(dir).catch(() => {});

      const unlisten = listen("index-updated", () => {
        indexStats(dir)
          .then((stats: any) => {
            setIndexStatus({
              fileCount: stats.fileCount ?? 0,
              symbolCount: stats.symbolCount ?? 0,
              state: "ready",
            });
          })
          .catch(() => {});
      });

      return () => {
        indexStopWatcher().catch(() => {});
        unlisten.then((fn) => fn());
      };
    } else {
      setIndexStatus({ fileCount: 0, symbolCount: 0, state: "building" });
      indexBuildRepo()
        .then((stats: any) => {
          setIndexStatus({
            fileCount: stats.fileCount ?? 0,
            symbolCount: stats.symbolCount ?? 0,
            state: "ready",
          });
        })
        .catch(() => setIndexStatus(null));
    }
  }, [project.projectDir, layout.workspaceMode]);

  // Theme effect
  useEffect(() => {
    const root = document.documentElement;
    if (layout.theme === "light") root.classList.add("light");
    else root.classList.remove("light");
    try { localStorage.setItem("modai-theme", layout.theme); } catch { /* ignore */ }
  }, [layout.theme]);

  const fontUiStack = layout.fontUi === "code"
    ? "Consolas, Monaco, \"Courier New\", \"Liberation Mono\", monospace"
    : "\"Microsoft JhengHei Light\", \"Microsoft JhengHei\", \"PingFang SC\", \"Hiragino Sans GB\", \"Microsoft YaHei\", system-ui, sans-serif";

  useEffect(() => {
    const root = document.documentElement;
    root.style.setProperty("--font-ui", fontUiStack);
    root.style.setProperty("--font-size-ui", `calc(12px * ${layout.fontSizePercent} / 100)`);
    try {
      localStorage.setItem("modai-font-ui", layout.fontUi);
      localStorage.setItem("modai-font-size-percent", String(layout.fontSizePercent));
    } catch { /* ignore */ }
  }, [layout.fontUi, layout.fontSizePercent, fontUiStack]);

  useEffect(() => {
    emit("frontend-ready").catch(() => {});
  }, []);

  useEffect(() => {
    const syncFontFromStorage = () => {
      const root = document.documentElement;
      const fontUi = localStorage.getItem("modai-font-ui") === "code" ? "code" : "chinese";
      const stack = fontUi === "code"
        ? "Consolas, Monaco, \"Courier New\", \"Liberation Mono\", monospace"
        : "\"Microsoft JhengHei Light\", \"Microsoft JhengHei\", \"PingFang SC\", \"Hiragino Sans GB\", \"Microsoft YaHei\", system-ui, sans-serif";
      root.style.setProperty("--font-ui", stack);
      const pct = localStorage.getItem("modai-font-size-percent");
      const n = pct ? parseInt(pct, 10) : 100;
      const percent = [90, 100, 110, 120].includes(n) ? n : 100;
      root.style.setProperty("--font-size-ui", `calc(12px * ${percent} / 100)`);
    };
    window.addEventListener("modai-font-ui-change", syncFontFromStorage);
    window.addEventListener("modai-font-size-change", syncFontFromStorage);
    return () => {
      window.removeEventListener("modai-font-ui-change", syncFontFromStorage);
      window.removeEventListener("modai-font-size-change", syncFontFromStorage);
    };
  }, []);

  useEffect(() => {
    const root = document.documentElement;
    if (layout.uiColorScheme === "classic") root.classList.add("ui-color-classic");
    else root.classList.remove("ui-color-classic");
    try { localStorage.setItem("modai-ui-color-scheme", layout.uiColorScheme); } catch { /* ignore */ }
  }, [layout.uiColorScheme]);

  useEffect(() => {
    const root = document.documentElement;
    root.style.setProperty("--panel-header-height", `${layout.panelHeaderHeight}px`);
    root.style.setProperty("--toolbar-btn-size", `${layout.toolbarBtnSize}px`);
    root.style.setProperty("--toolbar-gap", `${layout.toolbarGap}px`);
  }, [layout.panelHeaderHeight, layout.toolbarBtnSize, layout.toolbarGap]);

  useEffect(() => {
    const text = (currentSelection.text ?? "").trim();
    if (!text) {
      ai.setContextBlocks([]);
      return;
    }
    const p = (currentSelection.path ?? openFilePath ?? "selection").replace(/\\/g, "/");
    const block: AiContextBlock = { path: p, content: text };
    ai.setContextBlocks([block]);
  }, [currentSelection.path, currentSelection.text, openFilePath, ai.setContextBlocks]);

  const toggleLang = useCallback(() => {
    const next = layout.lang === "en" ? "zh" : "en";
    setLang(next);
    layout.setLang(next);
  }, [layout.lang]);

  const runIndexAction = useCallback(async (action: "refresh" | "rebuild") => {
    if (indexAction.running) return;
    setIndexAction({ running: true, action, done: 0, total: 0 });
    const unlisten = await listen("index-progress", (event: any) => {
      const { done, total } = event.payload as { done: number; total: number };
      setIndexAction((prev) => ({ ...prev, done, total }));
    });
    try {
      let stats: any;
      if (layout.workspaceMode === "modelica" && project.projectDir) {
        stats =
          action === "rebuild"
            ? await indexRebuild(project.projectDir)
            : await indexRefresh(project.projectDir);
      } else {
        stats = action === "rebuild" ? await indexRebuildRepo() : await indexRefreshRepo();
      }
      setIndexStatus({
        fileCount: stats.fileCount ?? 0,
        symbolCount: stats.symbolCount ?? 0,
        state: "ready",
      });
    } catch {
      // keep previous state
    } finally {
      unlisten();
      setIndexAction({ running: false, action: null, done: 0, total: 0 });
    }
  }, [indexAction.running, layout.workspaceMode, project.projectDir]);

  // Close project menu on outside click
  useEffect(() => {
    if (!layout.showProjectMenu && !showJitViewMenu) return;
    const closeMenus = () => {
      layout.setShowProjectMenu(false);
      setShowJitViewMenu(false);
    };
    const t = setTimeout(() => window.addEventListener("click", closeMenus), 0);
    return () => {
      clearTimeout(t);
      window.removeEventListener("click", closeMenus);
    };
  }, [layout.showProjectMenu, showJitViewMenu]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      if (e.shiftKey && (e.key === "F" || e.key === "f")) {
        e.preventDefault();
        layout.setShowLeftSidebar(true);
        layout.setLeftSidebarTab("search");
        return;
      }
      switch (e.key) {
        case "s":
          e.preventDefault();
          workbenchRef.current?.save();
          break;
        case "b":
          e.preventDefault();
          layout.setShowLeftSidebar(!layout.showLeftSidebar);
          break;
        case "j":
          e.preventDefault();
          layout.setShowBottomPanel(!layout.showBottomPanel);
          break;
        case "n":
          if (layout.workspaceMode === "modelica" && project.projectDir) {
            e.preventDefault();
            setShowNewModelDialog(true);
          }
          break;
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [layout.showLeftSidebar, layout.showBottomPanel]);

  const handleOpenRecentProject = useCallback(
    (path: string) => {
      project.setProjectDirFromPath(path).then(() => layout.setWorkspaceMode("modelica"));
    },
    [project.setProjectDirFromPath, layout.setWorkspaceMode]
  );

  const handleOpenMoFile = useCallback((relativePath: string, groupIndex?: number) => {
    workbenchRef.current?.openFile(relativePath, groupIndex);
  }, []);

  const handleCreateMoFile = useCallback(
    async (relativePath: string, content: string) => {
      if (!project.projectDir) throw new Error("No project directory");
      await writeProjectFile(project.projectDir, relativePath, content);
      handleOpenMoFile(relativePath);
    },
    [project.projectDir, handleOpenMoFile]
  );

  const handleValidate = useCallback(async () => {
    layout.setShowBottomPanel(true);
    setRequestedSimulationTab("problems");
    const result = await sim.validate(code, modelName, project.projectDir);
    if (result && !result.success) {
      ai.setAiPrompt(`Compilation failed.\n${result.errors.join("\n")}`);
      layout.setShowRightPanel(true);
      layout.setRightPanelTab("ai");
    }
  }, [code, modelName, project.projectDir, sim, layout, ai]);

  const handleRunSimulation = useCallback(async () => {
    layout.setShowBottomPanel(true);
    setRequestedSimulationTab("results");
    await sim.runSimulation(code, modelName, project.projectDir);
  }, [code, modelName, project.projectDir, sim, layout]);

  const handleTestAll = useCallback(async () => {
    if (project.projectDir && project.moFiles.length > 0) {
      layout.setShowBottomPanel(true);
      await sim.testAllMoFiles(project.projectDir, project.moFiles, pathToModelName);
      setRequestedSimulationTab("problems");
    }
  }, [project.projectDir, project.moFiles, sim, layout]);

  const isSettingsOpen = layout.showSettings;

  const handleOpenSettings = useCallback(() => {
    layout.setShowSettings(!layout.showSettings);
  }, [layout.showSettings, layout.setShowSettings]);

  const handleAppSettingsChange = useCallback(
    async (next: AppSettings) => {
      setAppSettingsState(next);
      try {
        await setAppSettings(next);
      } catch {
        /* ignore */
      }
    },
    []
  );

  const handleInsertAi = useCallback(() => {
    if (!editorRef.current) return;
    const editor = editorRef.current;
    const model = editor.getModel();
    if (!model) return;

    const pp = ai.pendingPatch;
    const textToInsert = pp?.newContent ?? ai.aiResponse;
    if (!textToInsert) return;

    if (pp?.startLine != null && pp?.endLine != null && pp.startLine >= 1 && pp.endLine >= 1) {
      const lineCount = model.getLineCount();
      const start = Math.min(pp.startLine, lineCount);
      const end = Math.min(pp.endLine, lineCount);
      if (start <= end) {
        const range = {
          startLineNumber: start,
          startColumn: 1,
          endLineNumber: end,
          endColumn: model.getLineMaxColumn(end),
        };
        editor.executeEdits("agent-edit-file", [{ range, text: textToInsert, forceMoveMarkers: true }]);
        ai.clearPendingPatch?.();
        return;
      }
    }

    const selection = editor.getSelection();
    const hasSelection = selection && !selection.isEmpty();
    if (hasSelection) {
      editor.executeEdits("agent-edit-selection", [
        { range: selection!, text: textToInsert, forceMoveMarkers: true },
      ]);
      ai.clearPendingPatch?.();
      return;
    }

    const pos = editor.getPosition();
    const range = pos
      ? { startLineNumber: pos.lineNumber, startColumn: pos.column, endLineNumber: pos.lineNumber, endColumn: pos.column }
      : { startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 1 };
    editor.executeEdits("insert-ai", [{ range, text: textToInsert, forceMoveMarkers: true }]);
    ai.clearPendingPatch?.();
  }, [ai.aiResponse, ai.pendingPatch, ai.clearPendingPatch]);

  const handleApplyDiff = useCallback(
    async (diff: string) => {
      const projectDir = project.projectDir;
      if (!projectDir) return;
      await applyPatchToProject(projectDir, diff);
      const paths = parsePathsFromDiff(diff);
      const normPaths = paths.map((p) => p.replace(/\\/g, "/")).filter((norm) => norm in contentByPath);
      if (normPaths.length === 0) return;
      const updates = await Promise.all(
        normPaths.map(async (norm) => ({ norm, content: await readProjectFile(projectDir, norm) }))
      );
      setContentByPath((prev) => {
        const next = { ...prev };
        for (const { norm, content } of updates) next[norm] = content;
        return next;
      });
    },
    [project.projectDir, contentByPath, setContentByPath]
  );

  return (
    <div className="flex flex-col h-screen bg-surface text-[var(--text)] overflow-hidden">
      <Titlebar
        workspaceMode={layout.workspaceMode}
        onWorkspaceModeChange={layout.setWorkspaceMode}
        modelName={modelName}
        showProjectMenu={layout.showProjectMenu}
        setShowProjectMenu={layout.setShowProjectMenu}
        isSettingsOpen={isSettingsOpen}
        onOpenSettings={handleOpenSettings}
        showLeftSidebar={layout.showLeftSidebar}
        setShowLeftSidebar={layout.setShowLeftSidebar}
        showRightPanel={layout.showRightPanel}
        setShowRightPanel={layout.setShowRightPanel}
        showBottomPanel={layout.showBottomPanel}
        setShowBottomPanel={layout.setShowBottomPanel}
        lang={layout.lang}
        onToggleLang={toggleLang}
        onOpenProject={project.openProject}
        recentProjects={recentProjects}
        onOpenRecentProject={handleOpenRecentProject}
        showJitViewMenu={showJitViewMenu}
        setShowJitViewMenu={setShowJitViewMenu}
        jitActiveCenterView={jitActiveCenterView}
        onJitCenterViewSelect={(view) => setJitCenterViewRequest(view)}
      />
      <div className="relative flex flex-1 min-h-0 min-w-0">
      <>
      <div className={layout.workspaceMode === "modelica" ? "flex flex-1 min-h-0 min-w-0" : "hidden"}>
            {layout.showLeftSidebar && (
              <>
                <div className="shrink-0 border-r border-border bg-surface-alt overflow-hidden flex flex-col w-full" style={{ width: layout.leftSidebarWidth }}>
                  <div className="panel-header-min-height shrink-0 flex items-center justify-around border-b border-border w-full">
                    <IconButton
                      icon={<AppIcon name="explorer" aria-hidden="true" />}
                      variant="tab"
                      size="xs"
                      active={layout.leftSidebarTab === "explorer"}
                      onClick={() => layout.setLeftSidebarTab("explorer")}
                      title={t("explorer")}
                      aria-label={t("explorer")}
                    />
                    <IconButton
                      icon={<AppIcon name="sourceControl" aria-hidden="true" />}
                      variant="tab"
                      size="xs"
                      active={layout.leftSidebarTab === "sourceControl"}
                      onClick={() => layout.setLeftSidebarTab("sourceControl")}
                      title={t("sourceControl")}
                      aria-label={t("sourceControl")}
                    />
                    <IconButton
                      icon={<AppIcon name="search" aria-hidden="true" />}
                      variant="tab"
                      size="xs"
                      active={layout.leftSidebarTab === "search"}
                      onClick={() => layout.setLeftSidebarTab("search")}
                      title={t("search")}
                      aria-label={t("search")}
                    />
                  </div>
                  <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
                    {layout.leftSidebarTab === "explorer" && (
                      <>
                        {project.projectDir && (
                          <div className="panel-header-bar shrink-0 flex items-center border-b border-border">
                            <button
                              type="button"
                              className="flex items-center gap-[var(--toolbar-gap)] px-2 py-0.5 text-[10px] rounded hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]"
                              onClick={() => setShowNewModelDialog(true)}
                              title={t("newModelTooltip")}
                            >
                              <span className="text-sm leading-none">+</span>
                              {t("newModel")}
                            </button>
                          </div>
                        )}
                        <div className="flex-1 min-h-0 overflow-auto scroll-vscode">
                          <FileTree
                            projectDir={project.projectDir}
                            moTree={project.moTree}
                            moFiles={project.moFiles}
                            onOpenProject={project.openProject}
                            onOpenFile={handleOpenMoFile}
                            recentProjects={recentProjects}
                            onOpenRecentProject={handleOpenRecentProject}
                            onNewModel={() => setShowNewModelDialog(true)}
                          />
                        </div>
                        <div className="shrink-0 max-h-64 overflow-auto scroll-vscode">
                          <OutlineSection
                            code={code}
                            openFilePath={openFilePath}
                            editorRef={editorRef}
                            projectDir={project.projectDir}
                            onOpenDiagram={() => workbenchRef.current?.setViewModeRequest?.("diagramReadOnly")}
                          />
                        </div>
                        <div className="shrink-0 max-h-64 overflow-auto scroll-vscode">
                          <TimelineSection
                            projectDir={project.projectDir}
                            openFilePath={openFilePath}
                            onOpenDiffAtRevision={(revision) => {
                              if (project.projectDir && openFilePath) {
                                project.setDiffTarget({
                                  projectDir: project.projectDir,
                                  relativePath: openFilePath,
                                  isStaged: false,
                                  revision,
                                });
                                layout.setRightPanelTab("diff");
                                layout.setShowRightPanel(true);
                              }
                            }}
                          />
                        </div>
                      </>
                    )}
                    {layout.leftSidebarTab === "sourceControl" && (
                      <div className="flex flex-col flex-1 min-h-0">
                        <div className="flex-1 min-h-0 overflow-hidden border-b border-border">
                          <SourceControlView
                            projectDir={project.projectDir}
                            onOpenDiff={(relativePath, isStaged) => {
                              if (project.projectDir) {
                                project.setDiffTarget({ projectDir: project.projectDir, relativePath, isStaged });
                                layout.setRightPanelTab("diff");
                                layout.setShowRightPanel(true);
                              }
                            }}
                            onOpenInEditor={handleOpenMoFile}
                            onRefreshStatus={project.refreshGitStatus}
                          />
                        </div>
                        <div className="shrink-0 border-t border-border flex flex-col min-h-0">
                          <button
                            type="button"
                            className="panel-header-bar shrink-0 flex items-center text-xs text-[var(--text-muted)] hover:bg-white/5 hover:text-[var(--text)] w-full text-left"
                            onClick={() => layout.setGraphExpanded((e) => !e)}
                            aria-expanded={layout.graphExpanded}
                          >
                            <span className="inline-block w-3 text-center" aria-hidden>{layout.graphExpanded ? "\u25BC" : "\u25B6"}</span>
                            {t("graph")}
                          </button>
                          {layout.graphExpanded && (
                            <div className="flex-1 min-h-[120px] overflow-hidden">
                              <Suspense fallback={<div className="p-2 text-[var(--text-muted)] text-xs">{t("loading")}</div>}>
                                <GitGraphView projectDir={project.projectDir} />
                              </Suspense>
                            </div>
                          )}
                        </div>
                      </div>
                    )}
                    {layout.leftSidebarTab === "search" && (
                      <SearchPanel
                        projectDir={project.projectDir}
                        onOpenFile={handleOpenMoFile}
                      />
                    )}
                  </div>
                </div>
                <div className="resize-handle shrink-0" onMouseDown={layout.startResizeLeft} aria-hidden />
              </>
            )}
            <div className="flex min-w-0 flex-col flex-1 min-h-0">
              {project.projectDir ? (
              <EditorWorkbench
                key={project.projectDir}
                ref={workbenchRef}
                projectDir={project.projectDir}
                gitStatus={project.gitStatus}
                jitResult={sim.jitResult}
                modelName={modelName}
                setModelName={setModelName}
                editorRef={editorRef}
                monacoRef={monacoRef}
                onFocusedChange={({ path, content }) => {
                  setOpenFilePath(path);
                  setCode(content);
                  ai.setActiveFilePath(path ?? null);
                }}
                onCursorPositionChange={(ln, col) => setCursorPosition({ lineNumber: ln, column: col })}
                onSelectionChange={({ path, selectedText }) => setCurrentSelection({ path, text: selectedText })}
                onGitStatusChange={project.setGitStatus}
                onContentByPathChange={setContentByPath}
                log={log}
                focusSymbolQuery={focusedDiagramSymbol}
                onRequestWorkbenchView={(view) => {
                  layout.setShowBottomPanel(true);
                  setRequestedSimulationTab(view === "analysis" ? "deps" : "results");
                }}
                onViewModeChange={(mode) => {
                  if (mode === "icon" || mode === "diagram" || mode === "diagramReadOnly") {
                    layout.setShowRightPanel(false);
                  }
                }}
                libraryRefreshToken={libraryRefreshToken}
                theme={layout.theme}
                initialEditorGroups={
                  restoredWorkspace?.projectDir === project.projectDir
                    ? (restoredWorkspace.meta.editorGroups as import("./components/EditorGroupColumn").EditorGroupState[])
                    : undefined
                }
                initialContentByPath={
                  restoredWorkspace?.projectDir === project.projectDir ? restoredWorkspace.drafts : undefined
                }
                initialFocusedGroupIndex={
                  restoredWorkspace?.projectDir === project.projectDir
                    ? restoredWorkspace.meta.focusedGroupIndex
                    : undefined
                }
                initialSplitRatio={
                  restoredWorkspace?.projectDir === project.projectDir ? restoredWorkspace.meta.splitRatio : undefined
                }
                initialProjectDir={
                  restoredWorkspace?.projectDir === project.projectDir ? project.projectDir : undefined
                }
              />
              ) : (
                <WelcomeView
                  onOpenProject={project.openProject}
                  recentProjects={recentProjects}
                  onOpenRecentProject={handleOpenRecentProject}
                />
              )}
              {layout.showBottomPanel && project.projectDir && (
                <>
                  <div className="resize-handle-h shrink-0" onMouseDown={layout.startResizeBottom} aria-hidden />
                  <div className="shrink-0 overflow-hidden flex flex-col border-t border-border bg-surface-alt" style={{ height: layout.bottomPanelHeight }}>
                    <div className="flex-1 min-h-0 overflow-hidden">
                      <Suspense fallback={<div className="p-3 text-[var(--text-muted)] text-sm">{t("loading")}</div>}>
                        <SimulationPanel
                          params={sim.params}
                          onParamChange={sim.setParam}
                          tableState={sim.tableState}
                          onTableChange={sim.setTable}
                          actions={{
                            onValidate: handleValidate,
                            onRunSimulation: handleRunSimulation,
                            onTestAll: handleTestAll,
                            onExportCSV: sim.exportCSV,
                            onExportJSON: sim.exportJSON,
                            onSuggestFixWithAi: ai.setAiPrompt,
                          onClearLog: () => setLogLines([]),
                          }}
                          data={{
                            jitResult: sim.jitResult,
                            simResult: sim.simResult,
                            simLoading: sim.simLoading,
                            testAllLoading: sim.testAllLoading,
                            testAllResults: sim.testAllResults,
                            moFilesCount: project.moFiles.length,
                            logLines,
                            plotSeries: sim.plotSeries,
                            chartMeta: sim.chartMeta,
                            allPlotVarNames: sim.allPlotVarNames,
                            selectedPlotVars: sim.selectedPlotVars,
                            tableColumns: sim.tableColumns,
                            sortedTableRows: sim.sortedTableRows,
                          }}
                          setSelectedPlotVars={sim.setSelectedPlotVars}
                          theme={layout.theme}
                          code={code}
                          openFilePath={openFilePath}
                          projectDir={project.projectDir}
                          requestedTab={requestedSimulationTab}
                          onRequestedTabHandled={() => setRequestedSimulationTab(null)}
                          onFocusSymbol={(symbol) => {
                            setFocusedDiagramSymbol(symbol);
                            layout.setShowBottomPanel(true);
                          }}
                          selectedSymbol={focusedDiagramSymbol}
                        />
                      </Suspense>
                    </div>
                  </div>
                </>
              )}
            </div>
            {layout.showRightPanel && (
              <>
                <div className="resize-handle shrink-0" onMouseDown={layout.startResizeRight} aria-hidden />
                <aside
                  className="shrink-0 border-l border-border bg-surface-alt overflow-hidden flex flex-col min-w-0 w-full"
                  style={{ width: layout.rightPanelWidth }}
                >
                  <div className="panel-header-min-height shrink-0 flex items-center justify-around border-b border-border w-full">
                    <IconButton
                      icon={<AppIcon name="ai" aria-hidden="true" />}
                      variant="tab"
                      size="xs"
                      active={layout.rightPanelTab === "ai"}
                      onClick={() => layout.setRightPanelTab("ai")}
                      title={t("aiCoding")}
                      aria-label={t("aiCoding")}
                    />
                    <IconButton
                      icon={<AppIcon name="diff" aria-hidden="true" />}
                      variant="tab"
                      size="xs"
                      active={layout.rightPanelTab === "diff"}
                      onClick={() => layout.setRightPanelTab("diff")}
                      title={t("viewDiff")}
                      aria-label={t("viewDiff")}
                    />
                  </div>
                  <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
                    {layout.rightPanelTab === "ai" && (
                      <div className="flex-1 overflow-auto p-3 scroll-vscode">
                        <AIPanel
                          apiKey={ai.apiKey}
                          setApiKey={ai.setApiKey}
                          apiKeySaved={ai.apiKeySaved}
                          onSaveApiKey={ai.saveApiKey}
                          aiPrompt={ai.aiPrompt}
                          setAiPrompt={ai.setAiPrompt}
                          aiLoading={ai.aiLoading}
                          aiResponse={ai.aiResponse}
                          onSend={ai.send}
                          onInsert={handleInsertAi}
                          tokenEstimate={ai.tokenEstimate}
                          dailyTokenUsed={ai.dailyTokenUsed}
                          dailyTokenLimit={ai.dailyTokenLimit}
                          sendDisabled={ai.sendDisabled}
                          projectDir={project.projectDir}
                          repoRoot={repoRoot}
                          mode={ai.mode}
                          setMode={ai.setMode}
                          model={ai.model}
                          setModel={ai.setModel}
                          currentFilePath={openFilePath ?? undefined}
                          currentSelectionText={currentSelection.text ?? undefined}
                          lastJitErrorText={sim.jitResult?.errors?.join(" ") ?? undefined}
                          messages={ai.messages}
                          agentMode={ai.agentMode}
                          setAgentMode={ai.setAgentMode}
                          pendingPatch={ai.pendingPatch}
                          clearPendingPatch={ai.clearPendingPatch}
                          onCreateMoFile={project.projectDir ? handleCreateMoFile : undefined}
                          onApplyDiff={project.projectDir ? handleApplyDiff : undefined}
                          iterationDiff={ai.iterationDiff}
                          iterationRunResult={ai.iterationRunResult}
                          iterationHistory={ai.iterationHistory}
                          onRunIteration={ai.runIteration}
                          onAdoptIteration={ai.adoptIteration}
                          onCommitIteration={ai.commitIteration}
                          onReuseIteration={ai.reuseIteration}
                          theme={layout.theme}
                        />
                      </div>
                    )}
                    {layout.rightPanelTab === "diff" && (
                      <Suspense fallback={<div className="p-3 text-[var(--text-muted)] text-sm">{t("loading")}</div>}>
                        <DiffView
                          diffTarget={project.diffTarget}
                          currentFileContent={
                            project.diffTarget ? (contentByPath[project.diffTarget.relativePath.replace(/\\/g, "/")] ?? null) : null
                          }
                          currentFilePath={openFilePath}
                          onClose={() => { project.setDiffTarget(null); layout.setRightPanelTab("ai"); }}
                          onOpenInEditor={(path) => handleOpenMoFile(path)}
                          theme={layout.theme}
                        />
                      </Suspense>
                    )}
                  </div>
                </aside>
              </>
            )}
          </div>
      <div className={layout.workspaceMode === "component-library" ? "flex flex-1 min-h-0 min-w-0" : "hidden"}>
        <ComponentLibraryWorkspace
          projectDir={project.projectDir}
          theme={layout.theme}
          onLibrariesChanged={() => setLibraryRefreshToken((value) => value + 1)}
          onOpenType={(typeName, libraryId) => {
            if (!project.projectDir) {
              return;
            }
            void workbenchRef.current?.openType(typeName, undefined, libraryId);
            layout.setWorkspaceMode("modelica");
          }}
        />
      </div>
      <div className={layout.workspaceMode === "compiler-iterate" ? "flex flex-1 min-h-0 min-w-0" : "hidden"}>
        <JitIdeWorkspace
          repoRoot={repoRoot}
          requestedCenterView={jitCenterViewRequest}
          onRequestedCenterViewHandled={() => setJitCenterViewRequest(undefined)}
          onActiveCenterViewChange={setJitActiveCenterView}
          theme={layout.theme}
        />
      </div>
      </>
      </div>
      <GlobalSettingsPanel
        open={layout.showSettings}
        onClose={() => layout.setShowSettings(false)}
        theme={layout.theme}
        onThemeChange={layout.setTheme}
        fontUi={layout.fontUi}
        onFontUiChange={layout.setFontUi}
        fontSizePercent={layout.fontSizePercent}
        onFontSizePercentChange={layout.setFontSizePercent}
        uiColorScheme={layout.uiColorScheme}
        onUiColorSchemeChange={layout.setUiColorScheme}
        panelHeaderHeight={layout.panelHeaderHeight}
        onPanelHeaderHeightChange={layout.setPanelHeaderHeight}
        toolbarBtnSize={layout.toolbarBtnSize}
        onToolbarBtnSizeChange={layout.setToolbarBtnSize}
        toolbarGap={layout.toolbarGap}
        onToolbarGapChange={layout.setToolbarGap}
        diagramSchemeId={diagramScheme.schemeId}
        diagramScheme={diagramScheme.scheme}
        onDiagramSchemeChange={diagramScheme.setSchemeId}
        indexFileCount={indexStatus?.fileCount ?? 0}
        indexSymbolCount={indexStatus?.symbolCount ?? 0}
        indexState={indexStatus?.state ?? null}
        indexAction={indexAction}
        onIndexRefresh={() => runIndexAction("refresh")}
        onIndexRebuild={() => runIndexAction("rebuild")}
        onEnterDevMode={() => layout.setWorkspaceMode("compiler-iterate")}
        aiModel={ai.model}
        onAiModelChange={ai.setModel}
        aiDailyUsed={ai.dailyTokenUsed}
        aiDailyLimit={ai.dailyTokenLimit}
        onAiDailyReset={ai.resetDailyUsage}
        defaultWorkspace={layout.defaultWorkspace}
        onDefaultWorkspaceChange={layout.setDefaultWorkspace}
        restoreLayout={layout.restoreLayout}
        onRestoreLayoutChange={layout.setRestoreLayout}
        lang={layout.lang}
        onLangChange={(l) => {
          setLang(l);
          layout.setLang(l);
        }}
        appDataRoot={appDataRoot ?? undefined}
        appSettings={appSettings ?? undefined}
        onAppSettingsChange={handleAppSettingsChange}
      />
      <StatusBar
        gitBranch={project.gitBranch}
        openFilePath={layout.workspaceMode === "modelica" ? openFilePath : null}
        language={layout.workspaceMode === "compiler-iterate" ? t("languageRust") : t("languageModelica")}
        position={layout.workspaceMode === "modelica" ? cursorPosition : null}
        errorCount={layout.workspaceMode === "modelica" ? (sim.jitResult?.errors?.length ?? 0) : 0}
        warningCount={layout.workspaceMode === "modelica" ? (sim.jitResult?.warnings?.length ?? 0) : 0}
        onBranchClick={() => {
          layout.setLeftSidebarTab("sourceControl");
          layout.setShowLeftSidebar(true);
        }}
        indexStatus={indexStatus}
      />
      <NewModelDialog
        projectDir={project.projectDir}
        open={showNewModelDialog}
        onClose={() => setShowNewModelDialog(false)}
        onCreateModel={handleCreateMoFile}
      />
    </div>
  );
}

export default App;
