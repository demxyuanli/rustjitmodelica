import { useState, useCallback, useRef, useEffect } from "react";
import type monaco from "monaco-editor";
import { invoke } from "@tauri-apps/api/core";
import { setLang } from "./i18n";
import type { JitValidateOptions, JitValidateResult, SimulationResult } from "./types";
import { Titlebar } from "./components/Titlebar";
import { FileTree } from "./components/FileTree";
import { CodeEditor } from "./components/CodeEditor";
import { SimulationPanel } from "./components/SimulationPanel";
import { AIPanel } from "./components/AIPanel";
import { Modals } from "./components/Modals";
import { SelfIterateUI } from "./components/SelfIterateUI";
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
  const [showLayoutMenu, setShowLayoutMenu] = useState(false);
  const [showJitFailModal, setShowJitFailModal] = useState(false);
  const [jitFailErrors, setJitFailErrors] = useState<string[]>([]);
  const [projectDir, setProjectDir] = useState<string | null>(null);
  const [moFiles, setMoFiles] = useState<string[]>([]);
  const [selfIterateTargetPrefill, setSelfIterateTargetPrefill] = useState<string | null>(null);
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
      const files = (await invoke("list_mo_files", { projectDir: dir })) as string[];
      setMoFiles(files);
    } catch (e) {
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
        const name = relativePath.replace(/\.mo$/i, "").split(/[/\\]/).pop() ?? "model";
        setModelName(name);
      } catch {}
    },
    [projectDir]
  );

  useEffect(() => {
    const closeMenus = () => {
      setShowProjectMenu(false);
      setShowLayoutMenu(false);
    };
    if (showProjectMenu || showLayoutMenu) {
      const t = setTimeout(() => window.addEventListener("click", closeMenus), 0);
      return () => {
        clearTimeout(t);
        window.removeEventListener("click", closeMenus);
      };
    }
  }, [showProjectMenu, showLayoutMenu]);

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
        code,
        modelName,
        options: opts,
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
  }, [code, modelName, tEnd, dt, solver, log]);

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
        code,
        modelName,
        options: opts,
      })) as SimulationResult;
      setSimResult(result);
      log("Simulation done. Points: " + (result.time?.length ?? 0));
    } catch (e) {
      log("Simulation error: " + String(e));
    } finally {
      setSimLoading(false);
    }
  }, [code, modelName, tEnd, dt, solver, log]);

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
    <div className="flex flex-col h-screen bg-surface text-[var(--text)] rounded-xl overflow-hidden">
      <Titlebar
        modelName={modelName}
        showProjectMenu={showProjectMenu}
        setShowProjectMenu={setShowProjectMenu}
        setShowSettings={setShowSettings}
        showLayoutMenu={showLayoutMenu}
        setShowLayoutMenu={setShowLayoutMenu}
        showLeftSidebar={showLeftSidebar}
        setShowLeftSidebar={setShowLeftSidebar}
        showRightPanel={showRightPanel}
        setShowRightPanel={setShowRightPanel}
        showBottomPanel={showBottomPanel}
        setShowBottomPanel={setShowBottomPanel}
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
          setShowRightPanel(true);
          setShowJitFailModal(false);
        }}
        showSettings={showSettings}
        onSettingsClose={() => setShowSettings(false)}
        theme={theme}
        onThemeChange={setTheme}
      />
      <div className="flex flex-1 min-h-0">
        {showLeftSidebar && (
          <FileTree
            projectDir={projectDir}
            moFiles={moFiles}
            onOpenProject={handleOpenProject}
            onOpenFile={handleOpenMoFile}
            lang={lang}
            onToggleLang={toggleLang}
          />
        )}
        <CodeEditor
          value={code}
          onChange={setCode}
          modelName={modelName}
          onModelNameChange={setModelName}
          jitResult={jitResult}
          editorRef={editorRef}
          monacoRef={monacoRef}
        />
        {showRightPanel && (
          <aside className="w-[360px] shrink-0 border-l border-border bg-surface-alt p-3 overflow-auto flex flex-col rounded-l-lg">
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
            <SelfIterateUI targetPrefill={selfIterateTargetPrefill} onClearPrefill={() => setSelfIterateTargetPrefill(null)} />
          </aside>
        )}
      </div>
      {showBottomPanel && (
        <SimulationPanel
          tEnd={tEnd}
          setTEnd={setTEnd}
          dt={dt}
          setDt={setDt}
          solver={solver}
          setSolver={setSolver}
          onValidate={handleValidate}
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
      )}
    </div>
  );
}

export default App;
