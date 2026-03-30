import { Suspense, lazy, type RefObject, type Dispatch, type SetStateAction } from "react";
import type monaco from "monaco-editor";
import { t } from "../i18n";
import type { AppSettings } from "../api/tauri";
import type { ModelicaLayoutApi } from "../hooks/useLayout";
import type { ModelicaProjectApi } from "../hooks/useProject";
import type { ModelicaSimulationApi } from "../hooks/useSimulation";
import type { ModelicaAiApi } from "../hooks/useAI";
import type { RestoredModelicaWorkspace } from "../hooks/useModelicaWorkspacePersistence";
import { EditorWorkbench, type EditorWorkbenchRef } from "./EditorWorkbench";
import { FileTree } from "./FileTree";
import { OutlineSection } from "./OutlineSection";
import { TimelineSection } from "./TimelineSection";
import { SourceControlView } from "./SourceControlView";
import { WelcomeView } from "./WelcomeView";
import { AIPanel } from "./AIPanel";
import { SearchPanel } from "./SearchPanel";
import { AppIcon } from "./Icon";
import { IconButton } from "./IconButton";

const DiffView = lazy(() => import("./DiffView").then((m) => ({ default: m.DiffView })));
const GitGraphView = lazy(() => import("./GitGraphView").then((m) => ({ default: m.GitGraphView })));
const SimulationPanel = lazy(() => import("./SimulationPanel").then((m) => ({ default: m.SimulationPanel })));

export type WorkbenchBottomTab = "problems" | "output" | "results" | "deps";

export interface ModelicaWorkbenchLayoutProps {
  layout: ModelicaLayoutApi;
  project: ModelicaProjectApi;
  sim: ModelicaSimulationApi;
  ai: ModelicaAiApi;
  workbenchRef: RefObject<EditorWorkbenchRef | null>;
  editorRef: RefObject<monaco.editor.IStandaloneCodeEditor | null>;
  monacoRef: RefObject<typeof monaco | null>;
  repoRoot: string | null;
  appSettings: AppSettings | null;
  restoredWorkspace: RestoredModelicaWorkspace | null;
  recentProjects: string[];
  modelName: string;
  setModelName: Dispatch<SetStateAction<string>>;
  openFilePath: string | null;
  code: string;
  setOpenFilePath: Dispatch<SetStateAction<string | null>>;
  setCode: Dispatch<SetStateAction<string>>;
  currentSelection: { path: string | null; text: string | null };
  setCurrentSelection: Dispatch<SetStateAction<{ path: string | null; text: string | null }>>;
  setCursorPosition: Dispatch<SetStateAction<{ lineNumber: number; column: number } | null>>;
  contentByPath: Record<string, string>;
  setContentByPath: Dispatch<SetStateAction<Record<string, string>>>;
  logLines: string[];
  setLogLines: Dispatch<SetStateAction<string[]>>;
  libraryRefreshToken: number;
  focusedDiagramSymbol: string | null;
  setFocusedDiagramSymbol: Dispatch<SetStateAction<string | null>>;
  requestedSimulationTab: WorkbenchBottomTab | null;
  setRequestedSimulationTab: Dispatch<SetStateAction<WorkbenchBottomTab | null>>;
  setShowNewModelDialog: Dispatch<SetStateAction<boolean>>;
  log: (msg: string) => void;
  handleOpenMoFile: (relativePath: string, groupIndex?: number) => void;
  handleCreateMoFile: (relativePath: string, content: string) => Promise<void>;
  handleValidate: () => Promise<void>;
  handleRunSimulation: () => Promise<void>;
  handleTestAll: () => Promise<void>;
  handleInsertAi: () => void;
  handleApplyDiff: (diff: string) => Promise<void>;
  handleOpenRecentProject: (path: string) => void;
}

export function ModelicaWorkbenchLayout(props: ModelicaWorkbenchLayoutProps) {
  const {
    layout,
    project,
    sim,
    ai,
    workbenchRef,
    editorRef,
    monacoRef,
    repoRoot,
    appSettings,
    restoredWorkspace,
    recentProjects,
    modelName,
    setModelName,
    openFilePath,
    code,
    setOpenFilePath,
    setCode,
    currentSelection,
    setCurrentSelection,
    setCursorPosition,
    contentByPath,
    setContentByPath,
    logLines,
    setLogLines,
    libraryRefreshToken,
    focusedDiagramSymbol,
    setFocusedDiagramSymbol,
    requestedSimulationTab,
    setRequestedSimulationTab,
    setShowNewModelDialog,
    log,
    handleOpenMoFile,
    handleCreateMoFile,
    handleValidate,
    handleRunSimulation,
    handleTestAll,
    handleInsertAi,
    handleApplyDiff,
    handleOpenRecentProject,
  } = props;

  return (
    <>
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
        onOpenDependencyGraphSettings={() => {
          layout.setShowSettings(true);
          layout.setOpenSettingsToGroup("dependency-graph");
        }}
        appSettings={appSettings}
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
            ? (restoredWorkspace.meta.editorGroups as import("./EditorGroupColumn").EditorGroupState[])
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
                  appSettings={appSettings}
                  onOpenDependencyGraphSettings={() => {
                    layout.setShowSettings(true);
                    layout.setOpenSettingsToGroup("dependency-graph");
                  }}
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
                    onAppendLogLines: (lines) =>
                      setLogLines((prev) => [
                        ...prev,
                        ...lines.map(
                          (msg) => `${new Date().toISOString().slice(11, 19)} ${msg}`,
                        ),
                      ]),
                  }}
                  data={{
                    jitResult: sim.jitResult,
                    simResult: sim.simResult,
                    validateLoading: sim.validateLoading,
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
                  onNewChat={ai.newChat}
                  sessions={ai.sessions}
                  onLoadSession={ai.loadSessionById}
                  onDeleteSession={ai.deleteSession}
                  lastToolCallsUsed={ai.lastToolCallsUsed}
                  onOpenRulesAndSkills={() => {
                    layout.setShowSettings(true);
                    layout.setOpenSettingsToGroup("ai-config");
                  }}
                  enabledModelIds={appSettings?.ai?.modelIdsEnabled ?? undefined}
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
    </>
  );
}
