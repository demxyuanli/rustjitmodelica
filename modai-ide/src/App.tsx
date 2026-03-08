import React, { useState, useCallback, useRef, useEffect, lazy, Suspense } from "react";
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
import { EditorWorkbench, type EditorWorkbenchRef } from "./components/EditorWorkbench";
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
  const [modelName, setModelName] = useState("BouncingBall");
  const [tEnd, setTEnd] = useState(2);
  const [dt, setDt] = useState(0.01);
  const [solver, setSolver] = useState("rk45");
  const [outputInterval, setOutputInterval] = useState(0.05);
  const [atol, setAtol] = useState(1e-6);
  const [rtol, setRtol] = useState(1e-3);
  const [jitResult, setJitResult] = useState<JitValidateResult | null>(null);
  const [simResult, setSimResult] = useState<SimulationResult | null>(null);
  const [selectedPlotVars, setSelectedPlotVars] = useState<string[]>([]);
  const [simLoading, setSimLoading] = useState(false);
  const [logLines, setLogLines] = useState<string[]>([]);
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<typeof monaco | null>(null);
  const workbenchRef = useRef<EditorWorkbenchRef>(null);
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
  const [contentByPath, setContentByPath] = useState<Record<string, string>>({});
  const [openFilePath, setOpenFilePath] = useState<string | null>(null);
  const [code, setCode] = useState(DEFAULT_MODEL);
  const [gitBranch, setGitBranch] = useState<string | null>(null);
  const [gitStatus, setGitStatus] = useState<{ modified: string[]; staged: string[] } | null>(null);
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
      setGitStatus(null);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const isRepo = (await invoke("git_is_repo", { projectDir })) as boolean;
        if (cancelled) return;
        if (!isRepo) {
          setGitBranch(null);
          setGitStatus(null);
          return;
        }
        const status = (await invoke("git_status", { projectDir })) as { branch: string; modified: string[]; staged: string[] };
        if (!cancelled) {
          setGitBranch(status.branch ?? null);
          setGitStatus({ modified: status.modified ?? [], staged: status.staged ?? [] });
        }
      } catch {
        if (!cancelled) {
          setGitBranch(null);
          setGitStatus(null);
        }
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

  const handleOpenMoFile = useCallback((relativePath: string, groupIndex?: number) => {
    workbenchRef.current?.openFile(relativePath, groupIndex);
  }, []);

  const log = useCallback((msg: string) => {
    setLogLines((prev) => [...prev, `${new Date().toISOString().slice(11, 19)} ${msg}`]);
  }, []);

  const refreshGitStatus = useCallback(async () => {
    if (!projectDir) return;
    try {
      const isRepo = (await invoke("git_is_repo", { projectDir })) as boolean;
      if (!isRepo) {
        setGitStatus(null);
        return;
      }
      const status = (await invoke("git_status", { projectDir })) as { modified: string[]; staged: string[] };
      setGitStatus({ modified: status.modified ?? [], staged: status.staged ?? [] });
    } catch {
      setGitStatus(null);
    }
  }, [projectDir]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault();
        workbenchRef.current?.save();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

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

  const handleValidate = useCallback(async () => {
    try {
      const opts: JitValidateOptions = {
        t_end: tEnd,
        dt,
        solver,
        output_interval: outputInterval,
        atol,
        rtol,
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
        setSelectedPlotVars((prev) => (prev.length ? prev : [...new Set([...(result.state_vars ?? []), ...(result.output_vars ?? [])])]));
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
  }, [code, modelName, tEnd, dt, solver, outputInterval, atol, rtol, log, projectDir]);

  const handleRunSimulation = useCallback(async () => {
    setSimLoading(true);
    setSimResult(null);
    log("Running simulation...");
    try {
      const opts: JitValidateOptions = {
        t_end: tEnd,
        dt,
        solver,
        output_interval: outputInterval,
        atol,
        rtol,
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
      setSelectedPlotVars(Object.keys(result.series).filter((k) => k !== "time"));
      log("Simulation done. Points: " + (result.time?.length ?? 0));
    } catch (e) {
      log("Simulation error: " + String(e));
    } finally {
      setSimLoading(false);
    }
  }, [code, modelName, tEnd, dt, solver, outputInterval, atol, rtol, log, projectDir]);

  const handleTestAllMoFiles = useCallback(async () => {
    if (!projectDir || moFiles.length === 0) return;
    setTestAllLoading(true);
    setTestAllResults(null);
    log(t("testAllRunning"));
    const opts: JitValidateOptions = { t_end: tEnd, dt, solver, output_interval: outputInterval, atol, rtol };
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
  }, [projectDir, moFiles, tEnd, dt, solver, outputInterval, atol, rtol, log]);

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
    ? selectedPlotVars
        .filter((name) => simResult.series[name] != null)
        .map((name) => ({
          x: simResult.time,
          y: simResult.series[name],
          type: "scatter" as const,
          mode: "lines" as const,
          name,
        }))
    : [];
  const allPlotVarNames =
    simResult != null
      ? Object.keys(simResult.series).filter((k) => k !== "time")
      : jitResult != null
        ? [...new Set([...(jitResult.state_vars ?? []), ...(jitResult.output_vars ?? [])])]
        : [];

  const [simViewMode, setSimViewMode] = useState<"chart" | "table">("chart");
  const [tableSortKey, setTableSortKey] = useState<string>("time");
  const [tableSortAsc, setTableSortAsc] = useState(true);
  const [tablePage, setTablePage] = useState(0);
  const [tablePageSize, setTablePageSize] = useState(100);
  const [visibleTableColumns, setVisibleTableColumns] = useState<string[]>([]);
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

  useEffect(() => {
    if (tableColumns.length > 0) {
      setVisibleTableColumns([...tableColumns]);
      setTablePage(0);
    } else {
      setVisibleTableColumns([]);
    }
  }, [tableColumns.join(",")]);

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
                          onOpenInEditor={handleOpenMoFile}
                          onRefreshStatus={refreshGitStatus}
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
        <EditorWorkbench
          ref={workbenchRef}
          projectDir={projectDir}
          gitStatus={gitStatus}
          jitResult={jitResult}
          modelName={modelName}
          setModelName={setModelName}
          editorRef={editorRef}
          monacoRef={monacoRef}
          onFocusedChange={({ path, content }) => {
            setOpenFilePath(path);
            setCode(content);
          }}
          onCursorPositionChange={(ln, col) => setCursorPosition({ lineNumber: ln, column: col })}
          onGitStatusChange={setGitStatus}
          onContentByPathChange={setContentByPath}
          log={log}
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
                      currentFileContent={
                        diffTarget ? (contentByPath[diffTarget.relativePath.replace(/\\/g, "/")] ?? null) : null
                      }
                      currentFilePath={openFilePath}
                      onClose={() => { setDiffTarget(null); setRightPanelTab("ai"); }}
                      onOpenInEditor={(path) => handleOpenMoFile(path)}
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
                  outputInterval={outputInterval}
                  setOutputInterval={setOutputInterval}
                  atol={atol}
                  setAtol={setAtol}
                  rtol={rtol}
                  setRtol={setRtol}
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
                  tablePage={tablePage}
                  setTablePage={setTablePage}
                  tablePageSize={tablePageSize}
                  setTablePageSize={setTablePageSize}
                  visibleTableColumns={visibleTableColumns}
                  setVisibleTableColumns={setVisibleTableColumns}
                  onExportCSV={handleExportCSV}
                  onExportJSON={handleExportJSON}
                  plotTraces={plotTraces}
                  onSuggestFixWithAi={setAiPrompt}
                  selectedPlotVars={selectedPlotVars}
                  setSelectedPlotVars={setSelectedPlotVars}
                  allPlotVarNames={allPlotVarNames}
                  theme={theme}
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
