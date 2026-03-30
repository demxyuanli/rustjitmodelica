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
import type { EditorWorkbenchRef } from "./components/EditorWorkbench";
import { ComponentLibraryWorkspace } from "./components/ComponentLibraryWorkspace";
import { StatusBar, type IndexStatusInfo } from "./components/StatusBar";
import { ModelicaWorkbenchLayout, type WorkbenchBottomTab } from "./components/ModelicaWorkbenchLayout";
import { emit } from "@tauri-apps/api/event";

const RegressionWorkspacePanel = lazy(() => import("./components/RegressionWorkspacePanel").then((m) => ({ default: m.RegressionWorkspacePanel })));
import { NewModelDialog } from "./components/diagram/NewModelDialog";
import { JitIdeWorkspace } from "./components/JitIdeWorkspace";
import { GlobalSettingsPanel } from "./components/GlobalSettingsPanel";
import type { IndexActionState } from "./components/SettingsContent";
import { t } from "./i18n";
import type { JitCenterView } from "./hooks/useJitLayout";
import { DEFAULT_MODEL_BOUNCING_BALL } from "./examples";
import {
  indexBuild,
  indexStats,
  indexStartWatcher,
  indexStopWatcher,
  indexBuildRepo,
  indexRefresh,
  indexRebuild,
  indexRefreshRepo,
  indexRebuildRepo,
  indexRepoStats,
  writeProjectFile,
  setAppSettings,
  applyPatchToProject,
  readProjectFile,
} from "./api/tauri";
import { parsePathsFromDiff } from "./components/ai/ai-markdown";
import type { AppSettings } from "./api/tauri";
import { useAppBootstrapEffects } from "./hooks/useAppBootstrapEffects";
import { useModelicaWorkspacePersistence } from "./hooks/useModelicaWorkspacePersistence";
import "./App.css";

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
  const sim = useSimulation(log, appSettings?.validation?.defaultTier ?? null);
  const ai = useModelicaAI(log);
  const [appDataRoot, setAppDataRoot] = useState<string | null>(null);

  useAppBootstrapEffects({
    setRepoRoot,
    setAppSettingsState,
    setAppDataRoot,
    setWorkspaceMode: layout.setWorkspaceMode,
    setProjectDirFromPath: project.setProjectDirFromPath,
  });

  const restoredWorkspace = useModelicaWorkspacePersistence(project.projectDir, workbenchRef);

  useEffect(() => {
    if (project.projectDir) addRecentProject(project.projectDir);
  }, [project.projectDir, addRecentProject]);

  useEffect(() => {
    ai.setProjectDir(project.projectDir ?? null);
  }, [ai.setProjectDir, project.projectDir]);

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
      let cancelled = false;
      const setupWatcher = () => {
        indexStartWatcher(dir).catch(() => {});
        const unlistenPromise = listen("index-updated", () => {
          indexStats(dir)
            .then((stats: any) => {
              if (cancelled) return;
              setIndexStatus({
                fileCount: stats.fileCount ?? 0,
                symbolCount: stats.symbolCount ?? 0,
                state: "ready",
              });
            })
            .catch(() => {});
        });
        return unlistenPromise;
      };

      let unlisten: Promise<() => void> | null = null;

      indexStats(dir)
        .then((stats: any) => {
          if (cancelled) {
            return;
          }
          const hasIndex = (stats?.fileCount ?? 0) > 0;
          if (hasIndex) {
            setIndexStatus({
              fileCount: stats.fileCount ?? 0,
              symbolCount: stats.symbolCount ?? 0,
              state: "ready",
            });
            unlisten = setupWatcher();
          } else {
            indexBuild(dir)
              .then((buildStats: any) => {
                if (cancelled) return;
                setIndexStatus({
                  fileCount: buildStats.fileCount ?? 0,
                  symbolCount: buildStats.symbolCount ?? 0,
                  state: "ready",
                });
                unlisten = setupWatcher();
              })
              .catch(() => {
                if (cancelled) return;
                setIndexStatus(null);
              });
          }
        })
        .catch(() => {
          if (cancelled) return;
          // fallback: build from scratch
          indexBuild(dir)
            .then((buildStats: any) => {
              if (cancelled) return;
              setIndexStatus({
                fileCount: buildStats.fileCount ?? 0,
                symbolCount: buildStats.symbolCount ?? 0,
                state: "ready",
              });
              unlisten = setupWatcher();
            })
            .catch(() => {
              if (cancelled) return;
              setIndexStatus(null);
            });
        });

      return () => {
        cancelled = true;
        indexStopWatcher().catch(() => {});
        if (unlisten) {
          unlisten.then((fn) => fn()).catch(() => {});
        }
      };
    } else {
      setIndexStatus({ fileCount: 0, symbolCount: 0, state: "building" });
      if (!repoRoot) {
        return;
      }
      indexRepoStats()
        .then((stats: any) => {
          const hasIndex = (stats?.fileCount ?? 0) > 0;
          if (hasIndex) {
            setIndexStatus({
              fileCount: stats.fileCount ?? 0,
              symbolCount: stats.symbolCount ?? 0,
              state: "ready",
            });
            if (appSettings?.indexCache?.repoIndexRefreshOnJitLoad !== false) {
              indexRefreshRepo().catch(() => {});
            }
          } else {
            indexBuildRepo()
              .then((buildStats: any) => {
                setIndexStatus({
                  fileCount: buildStats.fileCount ?? 0,
                  symbolCount: buildStats.symbolCount ?? 0,
                  state: "ready",
                });
              })
              .catch(() => setIndexStatus(null));
          }
        })
        .catch(() => {
          indexBuildRepo()
            .then((buildStats: any) => {
              setIndexStatus({
                fileCount: buildStats.fileCount ?? 0,
                symbolCount: buildStats.symbolCount ?? 0,
                state: "ready",
              });
            })
            .catch(() => setIndexStatus(null));
        });
    }
  }, [project.projectDir, layout.workspaceMode, repoRoot, appSettings]);

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
      await project.refreshMoTree?.();
      handleOpenMoFile(relativePath);
    },
    [project.projectDir, project.refreshMoTree, handleOpenMoFile]
  );

  const handleValidate = useCallback(async () => {
    layout.setShowBottomPanel(true);
    setRequestedSimulationTab("output");
    const result = await sim.validate(code, modelName, project.projectDir);
    if (result && !result.success) {
      setRequestedSimulationTab("problems");
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
      const firstPath = paths.map((p) => p.replace(/\\/g, "/"))[0];
      if (firstPath) handleOpenMoFile(firstPath);
    },
    [project.projectDir, contentByPath, setContentByPath, handleOpenMoFile]
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
        <ModelicaWorkbenchLayout
          layout={layout}
          project={project}
          sim={sim}
          ai={ai}
          workbenchRef={workbenchRef}
          editorRef={editorRef}
          monacoRef={monacoRef}
          repoRoot={repoRoot}
          appSettings={appSettings}
          restoredWorkspace={restoredWorkspace}
          recentProjects={recentProjects}
          modelName={modelName}
          setModelName={setModelName}
          openFilePath={openFilePath}
          code={code}
          setOpenFilePath={setOpenFilePath}
          setCode={setCode}
          currentSelection={currentSelection}
          setCurrentSelection={setCurrentSelection}
          setCursorPosition={setCursorPosition}
          contentByPath={contentByPath}
          setContentByPath={setContentByPath}
          logLines={logLines}
          setLogLines={setLogLines}
          libraryRefreshToken={libraryRefreshToken}
          focusedDiagramSymbol={focusedDiagramSymbol}
          setFocusedDiagramSymbol={setFocusedDiagramSymbol}
          requestedSimulationTab={requestedSimulationTab}
          setRequestedSimulationTab={setRequestedSimulationTab}
          setShowNewModelDialog={setShowNewModelDialog}
          log={log}
          handleOpenMoFile={handleOpenMoFile}
          handleCreateMoFile={handleCreateMoFile}
          handleValidate={handleValidate}
          handleRunSimulation={handleRunSimulation}
          handleTestAll={handleTestAll}
          handleInsertAi={handleInsertAi}
          handleApplyDiff={handleApplyDiff}
          handleOpenRecentProject={handleOpenRecentProject}
        />
      </div>
      <div className={layout.workspaceMode === "component-library" ? "flex flex-1 min-h-0 min-w-0" : "hidden"}>
        <ComponentLibraryWorkspace
          projectDir={project.projectDir}
          isActive={layout.workspaceMode === "component-library"}
          theme={layout.theme}
          appSettings={appSettings}
          onOpenDependencyGraphSettings={() => {
            layout.setShowSettings(true);
            layout.setOpenSettingsToGroup("dependency-graph");
          }}
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
          gitStatusThrottleMs={appSettings?.indexCache?.gitStatusThrottleMs ?? 2000}
          onOpenRulesAndSkills={() => {
            layout.setShowSettings(true);
            layout.setOpenSettingsToGroup("ai-config");
          }}
          enabledModelIds={appSettings?.ai?.modelIdsEnabled ?? undefined}
        />
      </div>
      <div className={layout.workspaceMode === "regression" ? "flex flex-1 min-h-0 min-w-0" : "hidden"}>
        <div className="flex flex-1 min-h-0 min-w-0 bg-surface">
          <Suspense fallback={<div className="p-3 text-[var(--text-muted)] text-sm">{t("loading")}</div>}>
            <RegressionWorkspacePanel theme={layout.theme} />
          </Suspense>
        </div>
      </div>
      </>
      </div>
      <GlobalSettingsPanel
        open={layout.showSettings}
        onClose={() => {
          layout.setShowSettings(false);
          layout.setOpenSettingsToGroup(null);
        }}
        initialGroupId={layout.openSettingsToGroup}
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
        projectDir={project.projectDir ?? undefined}
      />
      <StatusBar
        gitBranch={project.gitBranch}
        openFilePath={layout.workspaceMode === "modelica" ? openFilePath : null}
        language={layout.workspaceMode === "compiler-iterate" ? t("languageRust") : (layout.workspaceMode === "regression" ? t("testManagerTitle") : t("languageModelica"))}
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
