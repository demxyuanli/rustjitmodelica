import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useJitLayout } from "../hooks/useJitLayout";
import { useJitAI } from "../hooks/useAI";
import { JitLeftSidebar } from "./JitLeftSidebar";
import { JitEditorWorkbench, type OpenFileTab, type SettingsViewProps, type JitEditorWorkbenchRef } from "./JitEditorWorkbench";
import { JitRightPanel, type JitDiffTarget } from "./JitRightPanel";
import { JitBottomPanel } from "./JitBottomPanel";
import { loadTraceabilityConfig } from "../data/jit_regression_metadata";

interface TestRunResult {
  name: string;
  passed: boolean;
  exitCode: number;
  stdout: string;
  stderr: string;
  durationMs: number;
}

interface JitIdeWorkspaceProps {
  targetPrefill?: string | null;
  onClearPrefill?: () => void;
  repoRoot?: string | null;
  showSettings?: boolean;
  onSettingsHandled?: () => void;
  settingsProps?: SettingsViewProps;
}

export function JitIdeWorkspace({ targetPrefill, onClearPrefill, repoRoot, showSettings, onSettingsHandled, settingsProps }: JitIdeWorkspaceProps) {
  const layout = useJitLayout();

  const [openFiles, setOpenFiles] = useState<OpenFileTab[]>([]);
  const [activeFilePath, setActiveFilePath] = useState<string | null>(null);
  const [selectedSourcePath, setSelectedSourcePath] = useState<string | null>(null);
  const [selectedTestName, setSelectedTestName] = useState<string | null>(null);
  const [buildOutput, setBuildOutput] = useState<string[]>([]);
  const [testResults, setTestResults] = useState<TestRunResult[]>([]);
  const [diffOverlay, setDiffOverlay] = useState<string | null>(null);
  const [suiteRunning, setSuiteRunning] = useState(false);
  const [currentSelection, setCurrentSelection] = useState<{ path: string | null; text: string | null }>({ path: null, text: null });
  const [_gitStatus, setGitStatus] = useState<{ modified: string[]; staged: string[] } | null>(null);
  const [jitDiffTarget, setJitDiffTarget] = useState<JitDiffTarget | null>(null);

  const workbenchRef = useRef<JitEditorWorkbenchRef | null>(null);
  const jitLog = useCallback((msg: string) => {
    const ts = new Date().toISOString().slice(11, 19);
    setBuildOutput((prev) => [...prev, `[${ts}] ${msg}`]);
  }, []);
  const ai = useJitAI(jitLog);

  useEffect(() => {
    loadTraceabilityConfig().catch(() => {});
  }, []);

  useEffect(() => {
    if (targetPrefill) {
      layout.setRightTab("iterate");
      layout.setShowRightPanel(true);
    }
  }, [targetPrefill]);

  useEffect(() => {
    if (showSettings) {
      layout.setActiveCenterView("settings");
      onSettingsHandled?.();
    }
  }, [showSettings]);

  const refreshGitStatus = useCallback(async () => {
    if (!repoRoot) {
      setGitStatus(null);
      return;
    }
    try {
      const isRepo = (await invoke("git_is_repo", { projectDir: repoRoot })) as boolean;
      if (!isRepo) {
        setGitStatus(null);
        return;
      }
      const status = (await invoke("git_status", { projectDir: repoRoot })) as {
        modified?: string[];
        staged?: string[];
      };
      setGitStatus({
        modified: status.modified ?? [],
        staged: status.staged ?? [],
      });
    } catch {
      setGitStatus(null);
    }
  }, [repoRoot]);

  useEffect(() => {
    refreshGitStatus();
  }, [refreshGitStatus]);

  const openSourceFile = useCallback(async (path: string) => {
    setSelectedSourcePath(path);
    const existing = openFiles.find((f) => f.path === path);
    if (existing) {
      setActiveFilePath(path);
      return;
    }
    try {
      const content = await invoke<string>("read_compiler_file", { path });
      setOpenFiles((prev) => [...prev, { path, type: "rust", content, originalContent: content, dirty: false }]);
      setActiveFilePath(path);
    } catch {}
  }, [openFiles]);

  const openTestFile = useCallback(async (name: string) => {
    setSelectedTestName(name);
    const existing = openFiles.find((f) => f.path === name);
    if (existing) {
      setActiveFilePath(name);
      return;
    }
    try {
      const content = await invoke<string>("read_test_file", { name });
      setOpenFiles((prev) => [...prev, { path: name, type: "modelica", content, originalContent: content, dirty: false }]);
      setActiveFilePath(name);
    } catch {}
  }, [openFiles]);

  const handleFileContentChange = useCallback((path: string, content: string) => {
    setOpenFiles((prev) => prev.map((f) =>
      f.path === path ? { ...f, content, dirty: content !== f.originalContent } : f
    ));
  }, []);

  const handleFileSaved = useCallback((path: string) => {
    setOpenFiles((prev) => prev.map((f) =>
      f.path === path ? { ...f, originalContent: f.content, dirty: false } : f
    ));
    refreshGitStatus();
  }, [refreshGitStatus]);

  const handleFileClose = useCallback((path: string) => {
    const file = openFiles.find((f) => f.path === path);
    if (file?.dirty && !confirm("Unsaved changes. Discard?")) return;
    setOpenFiles((prev) => prev.filter((f) => f.path !== path));
    if (activeFilePath === path) {
      const remaining = openFiles.filter((f) => f.path !== path);
      setActiveFilePath(remaining.length > 0 ? remaining[remaining.length - 1].path : null);
    }
  }, [openFiles, activeFilePath]);

  const handleCreateTest = useCallback(async () => {
    const name = prompt("Test name (e.g. MyNewTest):");
    if (!name) return;
    const fullName = `TestLib/${name}`;
    const template = `model ${name}\n  Real x(start=0);\nequation\n  der(x) = 1;\nend ${name};\n`;
    try {
      await invoke("write_test_file", { name: fullName, content: template });
      openTestFile(fullName);
    } catch {}
  }, [openTestFile]);

  const handleTestRun = useCallback((name: string, result: TestRunResult) => {
    setTestResults((prev) => [result, ...prev.filter((r) => r.name !== name)]);
    layout.setBottomTab("testResults");
    layout.setShowBottomPanel(true);
  }, []);

  const handleRunSuite = useCallback(async (names: string[]) => {
    if (suiteRunning || names.length === 0) return;
    setSuiteRunning(true);
    layout.setBottomTab("testResults");
    layout.setShowBottomPanel(true);
    try {
      const result = await invoke<{
        total: number; passed: number; failed: number;
        results: TestRunResult[]; durationMs: number;
      }>("run_test_suite", { names, suite: "smoke" });
      setTestResults((prev) => [...result.results, ...prev]);
      setBuildOutput((prev) => [
        ...prev,
        `[${new Date().toISOString().slice(11, 19)}] Suite: ${result.passed}/${result.total} passed (${result.durationMs}ms)`,
      ]);
    } catch (e) {
      setBuildOutput((prev) => [...prev, `[${new Date().toISOString().slice(11, 19)}] Suite error: ${e}`]);
    } finally {
      setSuiteRunning(false);
    }
  }, [suiteRunning]);

  const handleDiffGenerated = useCallback((diff: string) => {
    setDiffOverlay(diff);
  }, []);

  const handleOpenDiff = useCallback((relativePath: string, isStaged: boolean) => {
    if (!repoRoot) return;
    setJitDiffTarget({ projectDir: repoRoot, relativePath, isStaged });
    layout.setRightTab("diff");
    layout.setShowRightPanel(true);
  }, [repoRoot]);

  const handleOpenInEditorFromDiff = useCallback((relativePath: string) => {
    if (relativePath.replace(/\\/g, "/").startsWith("TestLib/")) {
      openTestFile(relativePath);
    } else {
      openSourceFile(relativePath);
    }
  }, [openTestFile, openSourceFile]);

  const handleCloseDiff = useCallback(() => {
    setJitDiffTarget(null);
    layout.setRightTab("iterate");
  }, []);

  const handleViewIterationDiff = useCallback((iterationId: number, unifiedDiff: string, title?: string) => {
    setJitDiffTarget({ type: "iteration", iterationId, unifiedDiff, title });
    layout.setRightTab("diff");
    layout.setShowRightPanel(true);
  }, []);

  const contentByPath = openFiles.reduce<Record<string, string>>((acc, f) => {
    acc[f.path.replace(/\\/g, "/")] = f.content;
    return acc;
  }, {});

  const handleRunResult = useCallback((result: unknown) => {
    const r = result as {
      message?: string;
      success?: boolean;
      build_ok?: boolean;
      test_ok?: boolean;
      mo_run?: { passed: number; failed: number; details: Array<{ name: string; expected: string; actual: string }> } | null;
    };
    const ts = new Date().toISOString().slice(11, 19);
    if (r?.message) {
      setBuildOutput((prev) => [
        ...prev,
        `[${ts}] ${r.message}`,
        `[${ts}] build: ${r.build_ok ? "OK" : "FAIL"} | tests: ${r.test_ok ? "OK" : "FAIL"} | success: ${r.success ? "YES" : "NO"}`,
      ]);
    }
    if (r?.mo_run?.details) {
      const moResults: TestRunResult[] = r.mo_run.details.map((d) => ({
        name: d.name,
        passed: d.actual === d.expected,
        exitCode: d.actual === d.expected ? 0 : 1,
        stdout: `expected=${d.expected} actual=${d.actual}`,
        stderr: "",
        durationMs: 0,
      }));
      setTestResults((prev) => [...moResults, ...prev]);
    }
    layout.setBottomTab("output");
    layout.setShowBottomPanel(true);
  }, []);

  return (
    <div className="flex-1 min-h-0 overflow-hidden flex flex-col bg-surface">
      <div className="flex flex-1 min-h-0">
        {/* Left sidebar */}
        {layout.showLeftSidebar && (
          <>
            <div className="shrink-0 border-r border-border overflow-hidden flex flex-col" style={{ width: layout.leftSidebarWidth }}>
              <JitLeftSidebar
                activeTab={layout.leftTab}
                onTabChange={layout.setLeftTab}
                selectedSourcePath={selectedSourcePath}
                selectedTestName={selectedTestName}
                onSelectSource={openSourceFile}
                onSelectTest={openTestFile}
                onCreateTest={handleCreateTest}
                onRunSuite={handleRunSuite}
                suiteRunning={suiteRunning}
                repoRoot={repoRoot ?? undefined}
                onOpenDiff={handleOpenDiff}
                onOpenInEditor={handleOpenInEditorFromDiff}
                onRefreshGitStatus={refreshGitStatus}
              />
            </div>
            <div className="resize-handle shrink-0" onMouseDown={layout.startResizeLeft} aria-hidden />
          </>
        )}

        {/* Center: editor workbench + bottom panel */}
        <div className="flex-1 min-w-0 flex flex-col min-h-0">
          <div className="flex-1 min-h-0">
            <JitEditorWorkbench
              ref={workbenchRef}
              openFiles={openFiles}
              activeFilePath={activeFilePath}
              onActiveFileChange={setActiveFilePath}
              onFileContentChange={handleFileContentChange}
              onFileSaved={handleFileSaved}
              onFileClose={handleFileClose}
              onTestRun={handleTestRun}
              diffOverlay={diffOverlay}
              activeCenterView={layout.activeCenterView}
              onCenterViewChange={layout.setActiveCenterView}
              settingsProps={settingsProps}
              onSelectionChange={(sel) => setCurrentSelection(sel)}
              repoRoot={repoRoot ?? null}
            />
          </div>

          {layout.showBottomPanel && (
            <>
              <div className="resize-handle-h shrink-0" onMouseDown={layout.startResizeBottom} aria-hidden />
              <div
                className="shrink-0 overflow-hidden flex flex-col border-t border-border"
                style={{ height: layout.bottomPanelHeight }}
              >
                <JitBottomPanel
                  activeTab={layout.bottomTab}
                  onTabChange={layout.setBottomTab}
                  buildOutput={buildOutput}
                  testResults={testResults}
                />
              </div>
            </>
          )}
        </div>

        {/* Right panel */}
        {layout.showRightPanel && (
          <>
            <div className="resize-handle shrink-0" onMouseDown={layout.startResizeRight} aria-hidden />
            <aside className="shrink-0 border-l border-border overflow-hidden flex flex-col" style={{ width: layout.rightPanelWidth }}>
              <JitRightPanel
                activeTab={layout.rightTab}
                onTabChange={layout.setRightTab}
                targetPrefill={targetPrefill}
                onClearPrefill={onClearPrefill}
                repoRoot={repoRoot}
                onDiffGenerated={handleDiffGenerated}
                onRunResult={handleRunResult}
                openFilePaths={openFiles.filter((f) => f.type === "rust").map((f) => f.path)}
                jitDiffTarget={jitDiffTarget}
                onCloseDiff={handleCloseDiff}
                onOpenInEditor={handleOpenInEditorFromDiff}
                contentByPath={contentByPath}
                onViewIterationDiff={handleViewIterationDiff}
                aiPanelProps={{
                  apiKey: ai.apiKey,
                  setApiKey: ai.setApiKey,
                  apiKeySaved: ai.apiKeySaved,
                  onSaveApiKey: ai.saveApiKey,
                  aiPrompt: ai.aiPrompt,
                  setAiPrompt: ai.setAiPrompt,
                  aiLoading: ai.aiLoading,
                  aiResponse: ai.aiResponse,
                  onSend: ai.send,
                  onInsert: () => {},
                  tokenEstimate: ai.tokenEstimate,
                  dailyTokenUsed: ai.dailyTokenUsed,
                  dailyTokenLimit: ai.dailyTokenLimit,
                  sendDisabled: ai.sendDisabled,
                  projectDir: null,
                  repoRoot,
                  mode: ai.mode,
                  setMode: ai.setMode,
                  model: ai.model,
                  setModel: ai.setModel,
                  onCopyResult: undefined,
                  onOpenScratch: undefined,
                  currentFilePath: null,
                  currentSelectionText: null,
                  lastJitErrorText: undefined,
                }}
                currentFilePath={activeFilePath}
                currentSelectionText={currentSelection.text ?? null}
                onInsertAi={() => {
                  if (!ai.aiResponse) return;
                  workbenchRef.current?.insertAtCursor(ai.aiResponse);
                }}
              />
            </aside>
          </>
        )}
      </div>

    </div>
  );
}
