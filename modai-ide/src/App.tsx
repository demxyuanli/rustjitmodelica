import { useState, useCallback, useRef, useEffect, lazy, Suspense } from "react";
import type monaco from "monaco-editor";
import { listen } from "@tauri-apps/api/event";
import { setLang } from "./i18n";

import { useLayout } from "./hooks/useLayout";
import { useProject, pathToModelName } from "./hooks/useProject";
import { useSimulation } from "./hooks/useSimulation";
import { useModelicaAI, type AiContextBlock } from "./hooks/useAI";

import { Titlebar } from "./components/Titlebar";
import { FileTree } from "./components/FileTree";
import { OutlineSection } from "./components/OutlineSection";
import { TimelineSection } from "./components/TimelineSection";
import { SourceControlView } from "./components/SourceControlView";
import { EditorWorkbench, type EditorWorkbenchRef } from "./components/EditorWorkbench";
import { ComponentLibraryWorkspace } from "./components/ComponentLibraryWorkspace";
import { StatusBar, type IndexStatusInfo } from "./components/StatusBar";
import { AIPanel } from "./components/AIPanel";
import { SearchPanel } from "./components/SearchPanel";
import { AppIcon } from "./components/Icon";
import { IconButton } from "./components/IconButton";

const DiffView = lazy(() => import("./components/DiffView").then((m) => ({ default: m.DiffView })));
const GitGraphView = lazy(() => import("./components/GitGraphView").then((m) => ({ default: m.GitGraphView })));
const SimulationPanel = lazy(() => import("./components/SimulationPanel").then((m) => ({ default: m.SimulationPanel })));
import { JitIdeWorkspace } from "./components/JitIdeWorkspace";
import { SettingsContent, type IndexActionState } from "./components/SettingsContent";
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
} from "./api/tauri";
import "./App.css";

type WorkbenchBottomTab = "verify" | "run" | "log" | "deps" | "vars";

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

  useEffect(() => {
    indexRepoRoot().then((r) => setRepoRoot(r)).catch(() => {});
  }, []);

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
    layout.setLangState(next);
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
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [layout.showLeftSidebar, layout.showBottomPanel]);

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
    const result = await sim.validate(code, modelName, project.projectDir);
    if (result && !result.success) {
      ai.setAiPrompt(`Compilation failed.\n${result.errors.join("\n")}`);
      layout.setShowRightPanel(true);
      layout.setRightPanelTab("ai");
    }
  }, [code, modelName, project.projectDir, sim]);

  const handleRunSimulation = useCallback(() => {
    sim.runSimulation(code, modelName, project.projectDir);
  }, [code, modelName, project.projectDir, sim]);

  const handleTestAll = useCallback(() => {
    if (project.projectDir && project.moFiles.length > 0) {
      sim.testAllMoFiles(project.projectDir, project.moFiles, pathToModelName);
    }
  }, [project.projectDir, project.moFiles, sim]);

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

  return (
    <div className="flex flex-col h-screen bg-surface text-[var(--text)] overflow-hidden">
      <Titlebar
        workspaceMode={layout.workspaceMode}
        onWorkspaceModeChange={layout.setWorkspaceMode}
        modelName={modelName}
        showProjectMenu={layout.showProjectMenu}
        setShowProjectMenu={layout.setShowProjectMenu}
        onOpenSettings={() => {
          layout.setShowSettings(true);
        }}
        showLeftSidebar={layout.showLeftSidebar}
        setShowLeftSidebar={layout.setShowLeftSidebar}
        showRightPanel={layout.showRightPanel}
        setShowRightPanel={layout.setShowRightPanel}
        showBottomPanel={layout.showBottomPanel}
        setShowBottomPanel={layout.setShowBottomPanel}
        lang={layout.lang}
        onToggleLang={toggleLang}
        onOpenProject={project.openProject}
        showJitViewMenu={showJitViewMenu}
        setShowJitViewMenu={setShowJitViewMenu}
        jitActiveCenterView={jitActiveCenterView}
        onJitCenterViewSelect={(view) => setJitCenterViewRequest(view)}
      />
      {layout.workspaceMode === "modelica" ? (
        layout.showSettings ? (
          <div className="flex flex-col flex-1 min-h-0">
            <div className="flex-1 min-h-0 overflow-auto">
              <div className="max-w-2xl mx-auto p-6 text-[var(--text)]">
                <h2 className="text-lg font-semibold text-[var(--text)] mb-6">{t("settings")}</h2>
                <SettingsContent
                  theme={layout.theme}
                  onThemeChange={layout.setTheme}
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
                />
              </div>
            </div>
          </div>
        ) : (
          <div className="flex flex-1 min-h-0">
            {layout.showLeftSidebar && (
              <>
                <div className="shrink-0 border-r border-border bg-surface-alt overflow-hidden flex flex-col" style={{ width: layout.leftSidebarWidth }}>
                  <div className="shrink-0 flex border-b border-border justify-around py-0.5">
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
                  <div className="flex-1 min-h-0 overflow-auto flex flex-col scroll-vscode">
                    {layout.leftSidebarTab === "explorer" && (
                      <>
                        <FileTree
                          projectDir={project.projectDir}
                          moTree={project.moTree}
                          moFiles={project.moFiles}
                          onOpenProject={project.openProject}
                          onOpenFile={handleOpenMoFile}
                        />
                        <OutlineSection
                          code={code}
                          openFilePath={openFilePath}
                          editorRef={editorRef}
                          projectDir={project.projectDir}
                          onOpenDiagram={() => workbenchRef.current?.setViewModeRequest?.("diagramReadOnly")}
                        />
                        <TimelineSection
                          projectDir={project.projectDir}
                          openFilePath={openFilePath}
                          onOpenDiffAtRevision={(revision) => {
                            if (project.projectDir && openFilePath) {
                              project.setDiffTarget({ projectDir: project.projectDir, relativePath: openFilePath, isStaged: false, revision });
                              layout.setRightPanelTab("diff");
                              layout.setShowRightPanel(true);
                            }
                          }}
                        />
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
                            className="shrink-0 flex items-center gap-1 py-1.5 px-2 text-xs text-[var(--text-muted)] hover:bg-white/5 hover:text-[var(--text)] w-full text-left"
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
            <div className="flex flex-col flex-1 min-h-0">
              <EditorWorkbench
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
                  setRequestedSimulationTab(view === "analysis" ? "deps" : "run");
                }}
                onViewModeChange={(mode) => {
                  if (mode === "icon" || mode === "diagram" || mode === "diagramReadOnly") {
                    layout.setShowRightPanel(false);
                  }
                }}
                libraryRefreshToken={libraryRefreshToken}
                theme={layout.theme}
              />
              {layout.showBottomPanel && (
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
                          }}
                          data={{
                            jitResult: sim.jitResult,
                            simResult: sim.simResult,
                            simLoading: sim.simLoading,
                            testAllLoading: sim.testAllLoading,
                            testAllResults: sim.testAllResults,
                            moFilesCount: project.moFiles.length,
                            logLines,
                            plotTraces: sim.plotTraces,
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
                  className="shrink-0 border-l border-border bg-surface-alt overflow-hidden flex flex-col min-w-0"
                  style={{ width: layout.rightPanelWidth }}
                >
                  <div className="shrink-0 flex border-b border-border justify-around py-0.5">
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
                          iterationDiff={ai.iterationDiff}
                          iterationRunResult={ai.iterationRunResult}
                          iterationHistory={ai.iterationHistory}
                          onRunIteration={ai.runIteration}
                          onAdoptIteration={ai.adoptIteration}
                          onCommitIteration={ai.commitIteration}
                          onReuseIteration={ai.reuseIteration}
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
      )) : layout.workspaceMode === "component-library" ? (
        layout.showSettings ? (
          <div className="flex flex-col flex-1 min-h-0">
            <div className="flex-1 min-h-0 overflow-auto">
              <div className="max-w-2xl mx-auto p-6 text-[var(--text)]">
                <h2 className="text-lg font-semibold text-[var(--text)] mb-6">{t("settings")}</h2>
                <SettingsContent
                  theme={layout.theme}
                  onThemeChange={layout.setTheme}
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
                />
              </div>
            </div>
          </div>
        ) : (
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
        )
      ) : (
        <JitIdeWorkspace
          repoRoot={repoRoot}
          showSettings={layout.showSettings}
          onSettingsHandled={() => layout.setShowSettings(false)}
          requestedCenterView={jitCenterViewRequest}
          onRequestedCenterViewHandled={() => setJitCenterViewRequest(undefined)}
          onActiveCenterViewChange={setJitActiveCenterView}
          theme={layout.theme}
          settingsProps={{
            theme: layout.theme,
            onThemeChange: layout.setTheme,
            indexFileCount: indexStatus?.fileCount ?? 0,
            indexSymbolCount: indexStatus?.symbolCount ?? 0,
            indexState: indexStatus?.state ?? null,
            indexAction,
            onIndexRefresh: () => runIndexAction("refresh"),
            onIndexRebuild: () => runIndexAction("rebuild"),
          }}
        />
      )}
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
    </div>
  );
}

export default App;
