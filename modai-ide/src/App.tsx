import { useState, useCallback, useRef, useEffect, lazy, Suspense } from "react";
import type monaco from "monaco-editor";
import { invoke } from "@tauri-apps/api/core";
import { setLang } from "./i18n";
import type { JitValidateOptions, JitValidateResult, SimulationResult } from "./types";

export interface MoTreeEntry {
  name: string;
  path?: string;
  children?: MoTreeEntry[];
  class_name?: string;
  extends?: string[];
}

function flattenMoTree(node: MoTreeEntry): string[] {
  if (node.path) return [node.path];
  return (node.children ?? []).flatMap(flattenMoTree);
}
import { Titlebar } from "./components/Titlebar";
import { FileTree } from "./components/FileTree";
import { OutlineSection } from "./components/OutlineSection";
import { TimelineSection } from "./components/TimelineSection";
import { SourceControlView } from "./components/SourceControlView";
import { CodeEditor } from "./components/CodeEditor";
import { StatusBar } from "./components/StatusBar";
import { AIPanel } from "./components/AIPanel";

const DiffView = lazy(() => import("./components/DiffView").then((m) => ({ default: m.DiffView })));
const GitGraphView = lazy(() => import("./components/GitGraphView").then((m) => ({ default: m.GitGraphView })));
const SimulationPanel = lazy(() => import("./components/SimulationPanel").then((m) => ({ default: m.SimulationPanel })));
import { Modals } from "./components/Modals";
import { CompilerIterateWorkspace } from "./components/CompilerIterateWorkspace";
import { t } from "./i18n";
import "./App.css";

const DAILY_TOKEN_LIMIT = 50000;

function estimateTokens(text: string): number {
  return Math.ceil(text.length * 1.2);
}

function getDailyUsed(): number {
  try {
    const raw = localStorage.getItem("modai-ai-daily");
    if (!raw) return 0;
    const { date, used } = JSON.parse(raw) as { date: string; used: number };
    const today = new Date().toISOString().slice(0, 10);
    return date === today ? used : 0;
  } catch {
    return 0;
  }
}

function setDailyUsed(used: number): void {
  try {
    const date = new Date().toISOString().slice(0, 10);
    localStorage.setItem("modai-ai-daily", JSON.stringify({ date, used }));
  } catch {}
}

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
  const [code, setCode] = useState(DEFAULT_MODEL);
  const [modelName, setModelName] = useState("BouncingBall");
  const [tEnd, setTEnd] = useState(2);
  const [dt, setDt] = useState(0.01);
  const [solver, setSolver] = useState("rk45");
  const [jitResult, setJitResult] = useState<JitValidateResult | null>(null);
  const [simResult, setSimResult] = useState<SimulationResult | null>(null);
  const [simLoading, setSimLoading] = useState(false);
  const [logLines, setLogLines] = useState<string[]>([]);
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<typeof monaco | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [apiKeySaved, setApiKeySaved] = useState(false);
  const [aiPrompt, setAiPrompt] = useState("");
  const [aiLoading, setAiLoading] = useState(false);
  const [aiResponse, setAiResponse] = useState<string | null>(null);
  const [dailyTokenUsed, setDailyTokenUsed] = useState(getDailyUsed);
  const [lang, setLangState] = useState<"en" | "zh">("zh");
  const [theme, setTheme] = useState<"dark" | "light">(() => {
    try {
      const s = localStorage.getItem("modai-theme");
      return s === "light" ? "light" : "dark";
    } catch {
      return "dark";
    }
  });
  const [showLeftSidebar, setShowLeftSidebar] = useState(true);
  const [showRightPanel, setShowRightPanel] = useState(true);
  const [showBottomPanel, setShowBottomPanel] = useState(true);
  const [showProjectMenu, setShowProjectMenu] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [showJitFailModal, setShowJitFailModal] = useState(false);
  const [jitFailErrors, setJitFailErrors] = useState<string[]>([]);
  const [projectDir, setProjectDir] = useState<string | null>(null);
  const [moFiles, setMoFiles] = useState<string[]>([]);
  const [moTree, setMoTree] = useState<MoTreeEntry | null>(null);
  const [selfIterateTargetPrefill, setSelfIterateTargetPrefill] = useState<string | null>(null);
  const [workspaceMode, setWorkspaceMode] = useState<"modelica" | "compiler-iterate">("modelica");
  const [leftSidebarWidth, setLeftSidebarWidth] = useState(240);
  const [rightPanelWidth, setRightPanelWidth] = useState(360);
  const [bottomPanelHeight, setBottomPanelHeight] = useState(200);
  const [leftSidebarTab, setLeftSidebarTab] = useState<"explorer" | "sourceControl">("explorer");
  const [graphExpanded, setGraphExpanded] = useState(false);
  const [diffTarget, setDiffTarget] = useState<{ projectDir: string; relativePath: string; isStaged: boolean; revision?: string } | null>(null);
  const [rightPanelTab, setRightPanelTab] = useState<"ai" | "diff">("ai");
  const [openFilePath, setOpenFilePath] = useState<string | null>(null);
  const [gitBranch, setGitBranch] = useState<string | null>(null);
  const [cursorPosition, setCursorPosition] = useState<{ lineNumber: number; column: number } | null>(null);
  const [testAllLoading, setTestAllLoading] = useState(false);
  const [testAllResults, setTestAllResults] = useState<{ path: string; success: boolean; errors: string[] }[] | null>(null);
  const resizingRef = useRef<{ type: "left" | "right" | "bottom"; startX: number; startY: number; startSize: number } | null>(null);

  function pathToModelName(relativePath: string): string {
    return relativePath
      .replace(/\.mo$/i, "")
      .replace(/\\/g, "/")
      .split("/")
      .filter(Boolean)
      .join(".");
  }

  useEffect(() => {
    if (!projectDir) {
      setGitBranch(null);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const isRepo = (await invoke("git_is_repo", { projectDir })) as boolean;
        if (cancelled) return;
        if (!isRepo) {
          setGitBranch(null);
          return;
        }
        const status = (await invoke("git_status", { projectDir })) as { branch: string };
        if (!cancelled) setGitBranch(status.branch ?? null);
      } catch {
        if (!cancelled) setGitBranch(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [projectDir]);

  const startResizeLeft = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    resizingRef.current = { type: "left", startX: e.clientX, startY: 0, startSize: leftSidebarWidth };
    const onMove = (ev: MouseEvent) => {
      const r = resizingRef.current;
      if (!r || r.type !== "left") return;
      const delta = ev.clientX - r.startX;
      setLeftSidebarWidth(Math.min(480, Math.max(160, r.startSize + delta)));
    };
    const onUp = () => {
      resizingRef.current = null;
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }, [leftSidebarWidth]);

  const startResizeRight = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    resizingRef.current = { type: "right", startX: e.clientX, startY: 0, startSize: rightPanelWidth };
    const onMove = (ev: MouseEvent) => {
      const r = resizingRef.current;
      if (!r || r.type !== "right") return;
      const delta = ev.clientX - r.startX;
      setRightPanelWidth(Math.min(600, Math.max(280, r.startSize - delta)));
    };
    const onUp = () => {
      resizingRef.current = null;
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }, [rightPanelWidth]);

  const startResizeBottom = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    resizingRef.current = { type: "bottom", startX: 0, startY: e.clientY, startSize: bottomPanelHeight };
    const onMove = (ev: MouseEvent) => {
      const r = resizingRef.current;
      if (!r || r.type !== "bottom") return;
      const delta = ev.clientY - r.startY;
      setBottomPanelHeight(Math.min(400, Math.max(120, r.startSize - delta)));
    };
    const onUp = () => {
      resizingRef.current = null;
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }, [bottomPanelHeight]);

  useEffect(() => {
    const root = document.documentElement;
    if (theme === "light") root.classList.add("light");
    else root.classList.remove("light");
    try {
      localStorage.setItem("modai-theme", theme);
    } catch {}
  }, [theme]);
  const toggleLang = () => {
    const next = lang === "en" ? "zh" : "en";
    setLang(next);
    setLangState(next);
  };

  useEffect(() => {
    invoke("get_api_key")
      .then((k) => {
        setApiKey((k as string) ? "********" : "");
        setApiKeySaved(!!(k as string));
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    setDailyTokenUsed(getDailyUsed());
  }, []);

  const handleOpenProject = useCallback(async () => {
    try {
      const dir = (await invoke("open_project_dir")) as string | null;
      if (!dir) return;
      setProjectDir(dir);
      const tree = (await invoke("list_mo_tree", { projectDir: dir })) as MoTreeEntry;
      setMoTree(tree);
      setMoFiles(flattenMoTree(tree));
    } catch (e) {
      setMoTree(null);
      setMoFiles([]);
    }
  }, []);

  const handleOpenMoFile = useCallback(
    async (relativePath: string) => {
      if (!projectDir) return;
      try {
        const content = (await invoke("read_project_file", {
          projectDir,
          relativePath,
        })) as string;
        setCode(content);
        setOpenFilePath(relativePath);
        const name = relativePath.replace(/\.mo$/i, "").split(/[/\\]/).pop() ?? "model";
        setModelName(name);
      } catch {}
    },
    [projectDir]
  );

  useEffect(() => {
    const closeMenus = () => setShowProjectMenu(false);
    if (showProjectMenu) {
      const t = setTimeout(() => window.addEventListener("click", closeMenus), 0);
      return () => {
        clearTimeout(t);
        window.removeEventListener("click", closeMenus);
      };
    }
  }, [showProjectMenu]);

  const log = useCallback((msg: string) => {
    setLogLines((prev) => [...prev, `${new Date().toISOString().slice(11, 19)} ${msg}`]);
  }, []);

  const handleValidate = useCallback(async () => {
    try {
      const opts: JitValidateOptions = {
        t_end: tEnd,
        dt,
        solver,
        output_interval: 0.05,
      };
      const result = (await invoke("jit_validate", {
        request: {
          code,
          modelName,
          options: opts,
          projectDir: projectDir ?? undefined,
        },
      })) as JitValidateResult;
      setJitResult(result);
      if (result.success) {
        log("JIT validation OK");
        setShowJitFailModal(false);
      } else {
        log("JIT validation failed: " + result.errors.join("; "));
        setJitFailErrors(result.errors);
        setShowJitFailModal(true);
      }
    } catch (e) {
      log("Error: " + String(e));
      setJitResult(null);
      setShowJitFailModal(false);
    }
  }, [code, modelName, tEnd, dt, solver, log, projectDir]);

  const handleRunSimulation = useCallback(async () => {
    setSimLoading(true);
    setSimResult(null);
    log("Running simulation...");
    try {
      const opts: JitValidateOptions = {
        t_end: tEnd,
        dt,
        solver,
        output_interval: 0.05,
      };
      const result = (await invoke("run_simulation_cmd", {
        request: {
          code,
          modelName,
          options: opts,
          projectDir: projectDir ?? undefined,
        },
      })) as SimulationResult;
      setSimResult(result);
      log("Simulation done. Points: " + (result.time?.length ?? 0));
    } catch (e) {
      log("Simulation error: " + String(e));
    } finally {
      setSimLoading(false);
    }
  }, [code, modelName, tEnd, dt, solver, log, projectDir]);

  const handleTestAllMoFiles = useCallback(async () => {
    if (!projectDir || moFiles.length === 0) return;
    setTestAllLoading(true);
    setTestAllResults(null);
    log(t("testAllRunning"));
    const opts: JitValidateOptions = { t_end: tEnd, dt, solver, output_interval: 0.05 };
    const results: { path: string; success: boolean; errors: string[] }[] = [];
    for (const path of moFiles) {
      try {
        const content = (await invoke("read_project_file", { projectDir, relativePath: path })) as string;
        const modelName = pathToModelName(path);
        const result = (await invoke("jit_validate", {
          request: { code: content, modelName, options: opts, projectDir },
        })) as JitValidateResult;
        results.push({ path, success: result.success, errors: result.errors ?? [] });
      } catch (e) {
        results.push({ path, success: false, errors: [String(e)] });
      }
    }
    setTestAllResults(results);
    setTestAllLoading(false);
    const passed = results.filter((r) => r.success).length;
    const failed = results.filter((r) => !r.success).length;
    log(t("testAllSummary").replace("{passed}", String(passed)).replace("{failed}", String(failed)));
  }, [projectDir, moFiles, tEnd, dt, solver, log]);

  const handleSaveApiKey = useCallback(
    async (key: string) => {
      if (!key || key === "********") return;
      try {
        await invoke("set_api_key", { apiKey: key });
        setApiKeySaved(true);
        setApiKey("********");
        log("API key saved");
      } catch (e) {
        log("API key save error: " + String(e));
      }
    },
    [log]
  );

  const handleAiSend = useCallback(async () => {
    if (!aiPrompt.trim()) return;
    const est = estimateTokens(aiPrompt.trim());
    const used = getDailyUsed();
    if (used + est > DAILY_TOKEN_LIMIT) {
      log("Daily token limit reached. Used: " + used + ", limit: " + DAILY_TOKEN_LIMIT);
      return;
    }
    setAiLoading(true);
    setAiResponse(null);
    try {
      const result = (await invoke("ai_code_gen", { prompt: aiPrompt.trim() })) as string;
      setAiResponse(result);
      const newUsed = used + est + estimateTokens(result);
      setDailyUsed(newUsed);
      setDailyTokenUsed(newUsed);
      log("AI response received");
    } catch (e) {
      log("AI error: " + String(e));
      setAiResponse("Error: " + String(e));
    } finally {
      setAiLoading(false);
    }
  }, [aiPrompt, log]);

  const handleInsertAi = useCallback(() => {
    if (!aiResponse || !editorRef.current) return;
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
      { range, text: aiResponse, forceMoveMarkers: true },
    ]);
  }, [aiResponse]);

  const plotTraces = simResult
    ? Object.entries(simResult.series)
        .filter(([key]) => key !== "time")
        .map(([name, values]) => ({
          x: simResult.time,
          y: values,
          type: "scatter" as const,
          mode: "lines" as const,
          name,
        }))
    : [];

  const [simViewMode, setSimViewMode] = useState<"chart" | "table">("chart");
  const [tableSortKey, setTableSortKey] = useState<string>("time");
  const [tableSortAsc, setTableSortAsc] = useState(true);
  const tableColumns = simResult
    ? ["time", ...Object.keys(simResult.series).filter((k) => k !== "time")]
    : [];
  const tableRows = simResult
    ? simResult.time.map((_, i) => {
        const row: Record<string, number> = { time: simResult.time[i] };
        for (const k of Object.keys(simResult.series)) {
          row[k] = simResult.series[k][i];
        }
        return row;
      })
    : [];
  const sortedTableRows = [...tableRows].sort((a, b) => {
    const va = a[tableSortKey];
    const vb = b[tableSortKey];
    if (va == null || vb == null) return 0;
    const cmp = va < vb ? -1 : va > vb ? 1 : 0;
    return tableSortAsc ? cmp : -cmp;
  });

  const handleExportCSV = useCallback(() => {
    if (!simResult || tableColumns.length === 0) return;
    const header = tableColumns.join(",");
    const body = sortedTableRows.map((r) => tableColumns.map((c) => r[c]).join(",")).join("\n");
    const blob = new Blob([header + "\n" + body], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "simulation_result.csv";
    a.click();
    URL.revokeObjectURL(url);
  }, [simResult, tableColumns, sortedTableRows]);

  const handleExportJSON = useCallback(() => {
    if (!simResult || tableRows.length === 0) return;
    const blob = new Blob([JSON.stringify(tableRows, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "simulation_result.json";
    a.click();
    URL.revokeObjectURL(url);
  }, [simResult, tableRows]);

  return (
    <div className="flex flex-col h-screen bg-surface text-[var(--text)] overflow-hidden">
      <Titlebar
        workspaceMode={workspaceMode}
        onWorkspaceModeChange={setWorkspaceMode}
        modelName={modelName}
        showProjectMenu={showProjectMenu}
        setShowProjectMenu={setShowProjectMenu}
        setShowSettings={setShowSettings}
        showLeftSidebar={showLeftSidebar}
        setShowLeftSidebar={setShowLeftSidebar}
        showRightPanel={showRightPanel}
        setShowRightPanel={setShowRightPanel}
        showBottomPanel={showBottomPanel}
        setShowBottomPanel={setShowBottomPanel}
        lang={lang}
        onToggleLang={toggleLang}
      />
      <Modals
        showJitFailModal={showJitFailModal}
        jitFailErrors={jitFailErrors}
        onJitFailClose={() => setShowJitFailModal(false)}
        onJitFailYes={() => {
          setAiPrompt("Fix the following Modelica compile error and suggest corrected code: " + jitFailErrors.join(" "));
          setShowRightPanel(true);
          setShowJitFailModal(false);
        }}
        onJitFailTrySelfIterate={() => {
          setSelfIterateTargetPrefill("Fix compiler to support: " + jitFailErrors.join(" "));
          setWorkspaceMode("compiler-iterate");
          setShowJitFailModal(false);
        }}
        showSettings={showSettings}
        onSettingsClose={() => setShowSettings(false)}
        theme={theme}
        onThemeChange={setTheme}
      />
      {workspaceMode === "modelica" ? (
        <>
      <div className="flex flex-col flex-1 min-h-0">
        <div className="flex flex-1 min-h-0">
          {showLeftSidebar && (
            <>
              <div className="shrink-0 border-r border-border bg-surface-alt overflow-hidden flex flex-col" style={{ width: leftSidebarWidth }}>
                <div className="shrink-0 flex border-b border-border">
                  <button
                    type="button"
                    className={`flex-1 py-1.5 text-xs ${leftSidebarTab === "explorer" ? "bg-white/10 text-[var(--text)]" : "text-[var(--text-muted)] hover:bg-white/5"}`}
                    onClick={() => setLeftSidebarTab("explorer")}
                  >
                    {t("explorer")}
                  </button>
                  <button
                    type="button"
                    className={`flex-1 py-1.5 text-xs ${leftSidebarTab === "sourceControl" ? "bg-white/10 text-[var(--text)]" : "text-[var(--text-muted)] hover:bg-white/5"}`}
                    onClick={() => setLeftSidebarTab("sourceControl")}
                  >
                    {t("sourceControl")}
                  </button>
                </div>
                <div className="flex-1 min-h-0 overflow-auto flex flex-col scroll-vscode">
                  {leftSidebarTab === "explorer" && (
                    <>
                      <FileTree
                        projectDir={projectDir}
                        moTree={moTree}
                        moFiles={moFiles}
                        onOpenProject={handleOpenProject}
                        onOpenFile={handleOpenMoFile}
                      />
                      <OutlineSection
                        code={code}
                        openFilePath={openFilePath}
                        editorRef={editorRef}
                      />
                      <TimelineSection
                        projectDir={projectDir}
                        openFilePath={openFilePath}
                        onOpenDiffAtRevision={(revision) => {
                          if (projectDir && openFilePath) {
                            setDiffTarget({ projectDir, relativePath: openFilePath, isStaged: false, revision });
                            setRightPanelTab("diff");
                            setShowRightPanel(true);
                          }
                        }}
                      />
                    </>
                  )}
                  {leftSidebarTab === "sourceControl" && (
                    <div className="flex flex-col flex-1 min-h-0">
                      <div className="flex-1 min-h-0 overflow-hidden border-b border-border">
                        <SourceControlView
                          projectDir={projectDir}
                          onOpenDiff={(relativePath, isStaged) => {
                            if (projectDir) {
                              setDiffTarget({ projectDir, relativePath, isStaged });
                              setRightPanelTab("diff");
                              setShowRightPanel(true);
                            }
                          }}
                        />
                      </div>
                      <div className="shrink-0 border-t border-border flex flex-col min-h-0">
                        <button
                          type="button"
                          className="shrink-0 flex items-center gap-1 py-1.5 px-2 text-xs text-[var(--text-muted)] hover:bg-white/5 hover:text-[var(--text)] w-full text-left"
                          onClick={() => setGraphExpanded((e) => !e)}
                          aria-expanded={graphExpanded}
                        >
                          <span className="inline-block w-3 text-center" aria-hidden>{graphExpanded ? "\u25BC" : "\u25B6"}</span>
                          {t("graph")}
                        </button>
                        {graphExpanded && (
                          <div className="flex-1 min-h-[120px] overflow-hidden">
                            <Suspense fallback={<div className="p-2 text-[var(--text-muted)] text-xs">{t("loading")}</div>}>
                              <GitGraphView projectDir={projectDir} />
                            </Suspense>
                          </div>
                        )}
                      </div>
                    </div>
                  )}
                </div>
              </div>
              <div className="resize-handle shrink-0" onMouseDown={startResizeLeft} aria-hidden />
            </>
          )}
        <CodeEditor
          value={code}
          onChange={setCode}
          modelName={modelName}
          onModelNameChange={setModelName}
          jitResult={jitResult}
          editorRef={editorRef}
          monacoRef={monacoRef}
          onCursorPositionChange={(lineNumber, column) => setCursorPosition({ lineNumber, column })}
        />
        {showRightPanel && (
          <>
            <div className="resize-handle shrink-0" onMouseDown={startResizeRight} aria-hidden />
            <aside className="shrink-0 border-l border-border bg-surface-alt overflow-hidden flex flex-col" style={{ width: rightPanelWidth }}>
              <div className="shrink-0 flex border-b border-border">
                <button
                  type="button"
                  className={`flex-1 py-1.5 text-xs ${rightPanelTab === "ai" ? "bg-white/10 text-[var(--text)]" : "text-[var(--text-muted)] hover:bg-white/5"}`}
                  onClick={() => setRightPanelTab("ai")}
                >
                  AI
                </button>
                <button
                  type="button"
                  className={`flex-1 py-1.5 text-xs ${rightPanelTab === "diff" ? "bg-white/10 text-[var(--text)]" : "text-[var(--text-muted)] hover:bg-white/5"}`}
                  onClick={() => setRightPanelTab("diff")}
                >
                  {t("viewDiff")}
                </button>
              </div>
              <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
                {rightPanelTab === "ai" && (
                  <div className="flex-1 overflow-auto p-3 scroll-vscode">
                  <AIPanel
                    apiKey={apiKey}
                    setApiKey={setApiKey}
                    apiKeySaved={apiKeySaved}
                    onSaveApiKey={handleSaveApiKey}
                    aiPrompt={aiPrompt}
                    setAiPrompt={setAiPrompt}
                    aiLoading={aiLoading}
                    aiResponse={aiResponse}
                    onSend={handleAiSend}
                    onInsert={handleInsertAi}
                    tokenEstimate={estimateTokens(aiPrompt)}
                    dailyTokenUsed={dailyTokenUsed}
                    dailyTokenLimit={DAILY_TOKEN_LIMIT}
                    sendDisabled={aiLoading || !apiKeySaved || dailyTokenUsed + estimateTokens(aiPrompt.trim()) > DAILY_TOKEN_LIMIT}
                  />
                  </div>
                )}
                {rightPanelTab === "diff" && (
                  <Suspense fallback={<div className="p-3 text-[var(--text-muted)] text-sm">{t("loading")}</div>}>
                    <DiffView
                      diffTarget={diffTarget}
                      currentFileContent={diffTarget && openFilePath && diffTarget.relativePath.replace(/\\/g, "/") === openFilePath.replace(/\\/g, "/") ? code : null}
                      currentFilePath={openFilePath}
                      onClose={() => { setDiffTarget(null); setRightPanelTab("ai"); }}
                    />
                  </Suspense>
                )}
              </div>
            </aside>
          </>
        )}
      </div>
      {showBottomPanel && (
        <>
          <div className="resize-handle-h shrink-0" onMouseDown={startResizeBottom} aria-hidden />
          <div className="shrink-0 overflow-hidden flex flex-col border-t border-border bg-surface-alt" style={{ height: bottomPanelHeight }}>
            <div className="flex-1 min-h-0 overflow-hidden">
              <Suspense fallback={<div className="p-3 text-[var(--text-muted)] text-sm">{t("loading")}</div>}>
                <SimulationPanel
                  tEnd={tEnd}
                  setTEnd={setTEnd}
                  dt={dt}
                  setDt={setDt}
                  solver={solver}
                  setSolver={setSolver}
                  onValidate={handleValidate}
                  onTestAllMoFiles={handleTestAllMoFiles}
                  testAllLoading={testAllLoading}
                  testAllResults={testAllResults}
                  moFilesCount={moFiles.length}
                  onRunSimulation={handleRunSimulation}
                  simLoading={simLoading}
                  jitResult={jitResult}
                  logLines={logLines}
                  simResult={simResult}
                  simViewMode={simViewMode}
                  setSimViewMode={setSimViewMode}
                  tableSortKey={tableSortKey}
                  setTableSortKey={setTableSortKey}
                  tableSortAsc={tableSortAsc}
                  setTableSortAsc={setTableSortAsc}
                  tableColumns={tableColumns}
                  sortedTableRows={sortedTableRows}
                  onExportCSV={handleExportCSV}
                  onExportJSON={handleExportJSON}
                  plotTraces={plotTraces}
                  onSuggestFixWithAi={setAiPrompt}
                />
              </Suspense>
            </div>
          </div>
        </>
      )}
      <StatusBar
        gitBranch={gitBranch}
        openFilePath={openFilePath}
        language="Modelica"
        position={cursorPosition}
        errorCount={jitResult?.errors?.length ?? 0}
        warningCount={jitResult?.warnings?.length ?? 0}
        onBranchClick={() => {
          setLeftSidebarTab("sourceControl");
          setShowLeftSidebar(true);
        }}
      />
      </div>
        </>
      ) : (
        <CompilerIterateWorkspace
          targetPrefill={selfIterateTargetPrefill}
          onClearPrefill={() => setSelfIterateTargetPrefill(null)}
        />
      )}
    </div>
  );
}

export default App;
