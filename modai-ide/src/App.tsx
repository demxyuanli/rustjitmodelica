import { useState, useCallback, useRef, useEffect, lazy, Suspense } from "react";
import type monaco from "monaco-editor";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { setLang } from "./i18n";

import { useLayout } from "./hooks/useLayout";
import { useProject, pathToModelName } from "./hooks/useProject";
import { useSimulation } from "./hooks/useSimulation";
import { useModelicaAI } from "./hooks/useAI";

import { Titlebar } from "./components/Titlebar";
import { FileTree } from "./components/FileTree";
import { OutlineSection } from "./components/OutlineSection";
import { TimelineSection } from "./components/TimelineSection";
import { SourceControlView } from "./components/SourceControlView";
import { EditorWorkbench, type EditorWorkbenchRef } from "./components/EditorWorkbench";
import { StatusBar, type IndexStatusInfo } from "./components/StatusBar";
import { AIPanel } from "./components/AIPanel";
import { SearchPanel } from "./components/SearchPanel";
import { AppIcon } from "./components/Icon";
import { IconButton } from "./components/IconButton";

const DiffView = lazy(() => import("./components/DiffView").then((m) => ({ default: m.DiffView })));
const GitGraphView = lazy(() => import("./components/GitGraphView").then((m) => ({ default: m.GitGraphView })));
const SimulationPanel = lazy(() => import("./components/SimulationPanel").then((m) => ({ default: m.SimulationPanel })));
import { Modals } from "./components/Modals";
import { JitIdeWorkspace } from "./components/JitIdeWorkspace";
import { SettingsContent, type IndexActionState } from "./components/SettingsContent";
import { t } from "./i18n";
import "./App.css";

const DEFAULT_MODEL = `model BouncingBall
  Real h(start = 1);
  Real v(start = 0);
  parameter Real g = 9.81;
  parameter Real c = 0.9;
equation
  der(h) = v;
  der(v) = -g;
  when h <= 0 then
    reinit(v, -c * pre(v));
    reinit(h, 0);
  end when;
end BouncingBall;
`;

function App() {
  const [modelName, setModelName] = useState("BouncingBall");
  const [logLines, setLogLines] = useState<string[]>([]);
  const [contentByPath, setContentByPath] = useState<Record<string, string>>({});
  const [openFilePath, setOpenFilePath] = useState<string | null>(null);
  const [code, setCode] = useState(DEFAULT_MODEL);
  const [cursorPosition, setCursorPosition] = useState<{ lineNumber: number; column: number } | null>(null);
  const [showJitFailModal, setShowJitFailModal] = useState(false);
  const [jitFailErrors, setJitFailErrors] = useState<string[]>([]);
  const [selfIterateTargetPrefill, setSelfIterateTargetPrefill] = useState<string | null>(null);
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

  useEffect(() => {
    invoke("index_repo_root").then((r) => setRepoRoot(r as string)).catch(() => {});
  }, []);

  useEffect(() => {
    if (layout.workspaceMode === "modelica") {
      if (!project.projectDir) {
        setIndexStatus(null);
        return;
      }
      const dir = project.projectDir;
      setIndexStatus({ fileCount: 0, symbolCount: 0, state: "building" });
      invoke("index_build", { projectDir: dir })
        .then((stats: any) => {
          setIndexStatus({
            fileCount: stats.fileCount ?? 0,
            symbolCount: stats.symbolCount ?? 0,
            state: "ready",
          });
        })
        .catch(() => setIndexStatus(null));

      invoke("index_start_watcher", { projectDir: dir }).catch(() => {});

      const unlisten = listen("index-updated", () => {
        invoke("index_stats", { projectDir: dir })
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
        invoke("index_stop_watcher").catch(() => {});
        unlisten.then((fn) => fn());
      };
    } else {
      setIndexStatus({ fileCount: 0, symbolCount: 0, state: "building" });
      invoke("index_build_repo")
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
      let cmd: string;
      const args: Record<string, string> = {};
      if (layout.workspaceMode === "modelica" && project.projectDir) {
        cmd = action === "rebuild" ? "index_rebuild" : "index_refresh";
        args.projectDir = project.projectDir;
      } else {
        cmd = action === "rebuild" ? "index_rebuild_repo" : "index_refresh_repo";
      }
      const stats: any = await invoke(cmd, args);
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
    if (!layout.showProjectMenu) return;
    const closeMenus = () => layout.setShowProjectMenu(false);
    const t = setTimeout(() => window.addEventListener("click", closeMenus), 0);
    return () => {
      clearTimeout(t);
      window.removeEventListener("click", closeMenus);
    };
  }, [layout.showProjectMenu]);

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

  const handleValidate = useCallback(async () => {
    const result = await sim.validate(code, modelName, project.projectDir);
    if (result && !result.success) {
      setJitFailErrors(result.errors);
      setShowJitFailModal(true);
    } else {
      setShowJitFailModal(false);
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
    if (!ai.aiResponse || !editorRef.current) return;
    const model = editorRef.current.getModel();
    if (!model) return;
    const selection = editorRef.current.getSelection();
    const position = selection
      ? { lineNumber: selection.endLineNumber, column: selection.endColumn }
      : editorRef.current.getPosition() || { lineNumber: 1, column: 1 };
    const range = {
      startLineNumber: position.lineNumber,
      startColumn: position.column,
      endLineNumber: position.lineNumber,
      endColumn: position.column,
    };
    editorRef.current.executeEdits("insert-ai", [
      { range, text: ai.aiResponse, forceMoveMarkers: true },
    ]);
  }, [ai.aiResponse]);

  return (
    <div className="flex flex-col h-screen bg-surface text-[var(--text)] overflow-hidden">
      <Titlebar
        workspaceMode={layout.workspaceMode}
        onWorkspaceModeChange={layout.setWorkspaceMode}
        modelName={modelName}
        showProjectMenu={layout.showProjectMenu}
        setShowProjectMenu={layout.setShowProjectMenu}
        setShowSettings={layout.setShowSettings}
        showLeftSidebar={layout.showLeftSidebar}
        setShowLeftSidebar={layout.setShowLeftSidebar}
        showRightPanel={layout.showRightPanel}
        setShowRightPanel={layout.setShowRightPanel}
        showBottomPanel={layout.showBottomPanel}
        setShowBottomPanel={layout.setShowBottomPanel}
        lang={layout.lang}
        onToggleLang={toggleLang}
        onOpenProject={project.openProject}
      />
      <Modals
        showJitFailModal={showJitFailModal}
        jitFailErrors={jitFailErrors}
        onJitFailClose={() => setShowJitFailModal(false)}
        onJitFailYes={() => {
          ai.setAiPrompt("Fix the following Modelica compile error and suggest corrected code: " + jitFailErrors.join(" "));
          layout.setShowRightPanel(true);
          setShowJitFailModal(false);
        }}
        onJitFailTrySelfIterate={() => {
          setSelfIterateTargetPrefill("Fix compiler to support: " + jitFailErrors.join(" "));
          layout.setWorkspaceMode("compiler-iterate");
          setShowJitFailModal(false);
        }}
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
                }}
                onCursorPositionChange={(ln, col) => setCursorPosition({ lineNumber: ln, column: col })}
                onSelectionChange={({ path, selectedText }) => setCurrentSelection({ path, text: selectedText })}
                onGitStatusChange={project.setGitStatus}
                onContentByPathChange={setContentByPath}
                log={log}
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
                <aside className="shrink-0 border-l border-border bg-surface-alt overflow-hidden flex flex-col" style={{ width: layout.rightPanelWidth }}>
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
                        />
                      </Suspense>
                    )}
                  </div>
                </aside>
              </>
            )}
          </div>
      )) : (
        <JitIdeWorkspace
          targetPrefill={selfIterateTargetPrefill}
          onClearPrefill={() => setSelfIterateTargetPrefill(null)}
          repoRoot={repoRoot}
          showSettings={layout.showSettings}
          onSettingsHandled={() => layout.setShowSettings(false)}
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
        language={layout.workspaceMode === "modelica" ? "Modelica" : "Rust"}
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
