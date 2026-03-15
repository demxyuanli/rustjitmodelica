import React, { useState, useEffect, useCallback, useRef, forwardRef, useImperativeHandle, lazy, Suspense } from "react";
import { invoke } from "@tauri-apps/api/core";
import Editor from "@monaco-editor/react";
import type monaco from "monaco-editor";
import { t } from "../i18n";
import { getSourceModules, getCaseToSourceFiles } from "../data/jit_regression_metadata";
import type { JitCenterView } from "../hooks/useJitLayout";
import { useEditorDiffDecorations } from "../hooks/useEditorDiffDecorations";
import { AppIcon } from "./Icon";
import { IconButton } from "./IconButton";

const AnalyticsDashboard = lazy(() => import("./AnalyticsDashboard").then((m) => ({ default: m.AnalyticsDashboard })));
const TraceabilityView = lazy(() => import("./TraceabilityView").then((m) => ({ default: m.TraceabilityView })));
const JitOverview = lazy(() => import("./JitOverview").then((m) => ({ default: m.JitOverview })));
const FeatureCaseMap = lazy(() => import("./FeatureCaseMap").then((m) => ({ default: m.FeatureCaseMap })));

interface SymbolInfo {
  name: string;
  kind: string;
  lineStart: number;
  lineEnd: number;
  signature: string | null;
  docComment: string | null;
  filePath: string;
}

interface GitLogEntry {
  hash: string;
  short: string;
  author: string;
  date: string;
  message: string;
}

export interface OpenFileTab {
  path: string;
  type: "rust" | "modelica";
  content: string;
  originalContent: string;
  dirty: boolean;
}

interface TestRunResult {
  name: string;
  passed: boolean;
  exitCode: number;
  stdout: string;
  stderr: string;
  durationMs: number;
}

const CENTER_VIEW_META: Record<JitCenterView, { labelKey: string; icon: React.ReactNode }> = {
  analytics: { labelKey: "analyticsTitle", icon: <AppIcon name="chart" aria-hidden="true" /> },
  trace:     { labelKey: "traceabilityTitle", icon: <AppIcon name="sourceControl" aria-hidden="true" /> },
  overview:  { labelKey: "jitOverviewTitle", icon: <AppIcon name="columns" aria-hidden="true" /> },
  map:       { labelKey: "featureCaseMapTitle", icon: <AppIcon name="table" aria-hidden="true" /> },
};

export interface JitEditorWorkbenchRef {
  insertAtCursor: (text: string) => void;
}

export interface JitEditorWorkbenchProps {
  openFiles: OpenFileTab[];
  activeFilePath: string | null;
  onActiveFileChange: (path: string) => void;
  onFileContentChange: (path: string, content: string) => void;
  onFileSaved: (path: string) => void;
  onFileClose: (path: string) => void;
  onTestRun?: (name: string, result: TestRunResult) => void;
  diffOverlay?: string | null;
  activeCenterView: JitCenterView | null;
  onCenterViewChange: (view: JitCenterView | null) => void;
  onSelectionChange?: (params: { path: string | null; text: string | null }) => void;
  repoRoot?: string | null;
  theme?: "dark" | "light";
}

export const JitEditorWorkbench = forwardRef(function JitEditorWorkbench(
  {
    openFiles,
    activeFilePath,
    onActiveFileChange,
    onFileContentChange,
    onFileSaved,
    onFileClose,
    onTestRun,
    diffOverlay,
    activeCenterView,
    onCenterViewChange,
    onSelectionChange,
    repoRoot,
    theme = "dark",
  }: JitEditorWorkbenchProps,
  ref: React.ForwardedRef<JitEditorWorkbenchRef>
) {
  const [symbols, setSymbols] = useState<SymbolInfo[]>([]);
  const [showSymbols, setShowSymbols] = useState(false);
  const [gitLog, setGitLog] = useState<GitLogEntry[]>([]);
  const [saving, setSaving] = useState(false);
  const [running, setRunning] = useState(false);
  const [banner, setBanner] = useState<{ msg: string; type: "success" | "error" } | null>(null);
  const [editorReady, setEditorReady] = useState(false);
  const [isSplit, setIsSplit] = useState(false);
  const [splitRatio, setSplitRatio] = useState(0.5);
  const [focusedPane, setFocusedPane] = useState<0 | 1>(0);
  const splitResizeRef = useRef<{ startX: number; startRatio: number } | null>(null);

  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<typeof monaco | null>(null);
  const editorRefSecondary = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);

  const activeFile = openFiles.find((f) => f.path === activeFilePath) ?? null;
  const isRust = activeFile?.type === "rust";
  const isModelica = activeFile?.type === "modelica";

  const pathNorm = activeFilePath != null ? activeFilePath.replace(/\\/g, "/") : null;
  const activeContent = activeFile?.content ?? "";
  useEditorDiffDecorations(
    editorRef,
    monacoRef,
    repoRoot ?? undefined,
    isRust ? pathNorm : null,
    activeContent,
    editorReady
  );

  useImperativeHandle(
    ref,
    () => ({
      insertAtCursor(text: string) {
        const editor = isSplit ? (focusedPane === 0 ? editorRef.current : editorRefSecondary.current) : editorRef.current;
        if (!editor || !text || diffOverlay) return;
        const model = editor.getModel();
        if (!model) return;
        const selection = editor.getSelection();
        const position = selection
          ? { lineNumber: selection.endLineNumber, column: selection.endColumn }
          : editor.getPosition() || { lineNumber: 1, column: 1 };
        const range = {
          startLineNumber: position.lineNumber,
          startColumn: position.column,
          endLineNumber: position.lineNumber,
          endColumn: position.column,
        };
        editor.executeEdits("jit-ai-insert", [{ range, text, forceMoveMarkers: true }]);
      },
    }),
    [diffOverlay, isSplit, focusedPane]
  );

  useEffect(() => {
    if (!activeFilePath) { setSymbols([]); setGitLog([]); return; }
    setSymbols([]);
    setGitLog([]);
    if (isRust) {
      invoke<SymbolInfo[]>("index_repo_file_symbols", { filePath: activeFilePath }).then(setSymbols).catch(() => {});
      invoke<GitLogEntry[]>("compiler_file_git_log", { path: activeFilePath, limit: 10 }).then(setGitLog).catch(() => {});
    }
  }, [activeFilePath, isRust]);

  useEffect(() => {
    if (banner?.type === "success") {
      const tm = setTimeout(() => setBanner(null), 3000);
      return () => clearTimeout(tm);
    }
  }, [banner]);

  useEffect(() => {
    if (!isSplit) {
      editorRefSecondary.current = null;
      setFocusedPane(0);
    }
  }, [isSplit]);

  const handleSave = useCallback(async () => {
    if (!activeFile || !activeFile.dirty) return;
    setSaving(true);
    try {
      if (activeFile.type === "rust") {
        await invoke("write_compiler_file", { path: activeFile.path, content: activeFile.content });
      } else {
        await invoke("write_test_file", { name: activeFile.path, content: activeFile.content });
      }
      onFileSaved(activeFile.path);
      setBanner({ msg: "File saved", type: "success" });
    } catch (e) {
      setBanner({ msg: String(e), type: "error" });
    } finally {
      setSaving(false);
    }
  }, [activeFile, onFileSaved]);

  const handleRevert = useCallback(() => {
    if (!activeFile) return;
    onFileContentChange(activeFile.path, activeFile.originalContent);
  }, [activeFile, onFileContentChange]);

  const handleRunTest = useCallback(async () => {
    if (!activeFile || !isModelica) return;
    setRunning(true);
    try {
      const result = await invoke<TestRunResult>("run_single_test", { name: activeFile.path });
      onTestRun?.(activeFile.path, result);
    } catch (e) {
      onTestRun?.(activeFile.path, { name: activeFile.path, passed: false, exitCode: -1, stdout: "", stderr: String(e), durationMs: 0 });
    } finally {
      setRunning(false);
    }
  }, [activeFile, isModelica, onTestRun]);

  const startResizeSplit = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    splitResizeRef.current = { startX: e.clientX, startRatio: splitRatio };
    const onMove = (ev: MouseEvent) => {
      if (!splitResizeRef.current) return;
      const container = document.querySelector(".jit-editor-workbench");
      if (!container) return;
      const rect = container.getBoundingClientRect();
      const delta = (ev.clientX - splitResizeRef.current.startX) / rect.width;
      setSplitRatio(Math.min(0.8, Math.max(0.2, splitResizeRef.current.startRatio + delta)));
    };
    const onUp = () => {
      splitResizeRef.current = null;
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }, [splitRatio]);

  const moduleInfo = activeFilePath && isRust ? getSourceModules()[activeFilePath] : undefined;
  const linkedCases: string[] = [];
  if (activeFilePath && isRust) {
    for (const [caseName, sources] of Object.entries(getCaseToSourceFiles())) {
      if (sources.includes(activeFilePath)) linkedCases.push(caseName);
    }
  }

  const kindColor: Record<string, string> = {
    function: "text-yellow-300", struct: "text-blue-300", enum: "text-purple-300",
    impl: "text-cyan-300", trait: "text-green-300", mod: "text-orange-300",
    const: "text-pink-300", type: "text-teal-300",
  };

  const showingEditor = activeCenterView === null;

  return (
    <div className="flex flex-col h-full min-h-0 overflow-hidden">
      {banner && (
        <div className={`panel-header-bar shrink-0 flex items-center border-b border-border px-4 ${banner.type === "error" ? "theme-banner-danger" : "theme-banner-success"}`}>
          <span className="text-xs">{banner.msg}</span>
        </div>
      )}

      {/* Tab bar: file tabs + active center view tab + split/close split (editor only, like modai) */}
      <div className="panel-header-min-height flex items-center border-b border-border shrink-0 bg-[var(--surface-muted)] overflow-x-auto min-w-0">
        {openFiles.map((f) => {
          const isActive = showingEditor && f.path === activeFilePath;
          const label = f.type === "modelica" ? f.path.replace("TestLib/", "") : f.path.replace("src/", "");
          return (
            <div key={f.path}
              className={`flex items-center gap-[var(--toolbar-gap)] px-3 py-1.5 text-xs cursor-pointer shrink-0 ${
                isActive
                  ? "bg-[var(--surface)] text-[var(--text)]"
                  : "bg-[var(--surface-alt)] text-[var(--text-muted)] hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
              }`}
              onClick={() => { onCenterViewChange(null); onActiveFileChange(f.path); }}>
              <span className={`w-2 h-2 rounded-full shrink-0 ${f.type === "rust" ? "bg-orange-500" : "bg-blue-500"}`} />
              <span className="truncate max-w-[140px]" title={f.path}>{label}</span>
              {f.dirty && <span className="text-amber-400 ml-0.5">*</span>}
              <button type="button" className="ml-1 text-[var(--text-muted)] hover:text-[var(--text)] text-[10px]"
                onClick={(e) => { e.stopPropagation(); onFileClose(f.path); }}
                title={t("closeTab")}>&#215;</button>
            </div>
          );
        })}

        {activeCenterView && (
          <div className="flex items-center gap-[var(--toolbar-gap)] px-3 py-1.5 text-xs cursor-pointer shrink-0 bg-[var(--surface)] text-[var(--text)]">
            <span className="text-primary font-medium">
              {t(CENTER_VIEW_META[activeCenterView].labelKey as Parameters<typeof t>[0])}
            </span>
            <button type="button" className="ml-1 text-[var(--text-muted)] hover:text-[var(--text)] text-[10px]"
              onClick={() => onCenterViewChange(null)}
              title={t("closeTab")}>&#215;</button>
          </div>
        )}

        {/* Split/close split in tab bar (same row as file tabs), only when editor is shown - align with modai */}
        {showingEditor && activeFile && !isSplit && (
          <IconButton
            icon={<AppIcon name="columns" aria-hidden="true" />}
            variant="ghost"
            size="xs"
            className="shrink-0 ml-auto"
            onClick={(e) => { e.stopPropagation(); setIsSplit(true); setFocusedPane(1); }}
            title={t("splitEditor")}
            aria-label={t("splitEditor")}
          />
        )}
        {showingEditor && isSplit && (
          <IconButton
            icon={<AppIcon name="close" aria-hidden="true" />}
            variant="ghost"
            size="xs"
            className="shrink-0 ml-auto"
            onClick={(e) => { e.stopPropagation(); setIsSplit(false); }}
            title={t("closeSplit")}
            aria-label={t("closeSplit")}
          />
        )}
      </div>

      {/* Main content: editor or analysis view */}
      {activeCenterView ? (
        <div className="flex-1 min-h-0 overflow-auto">
          <Suspense fallback={<div className="p-4 text-[var(--text-muted)] text-xs">{t("loading")}</div>}>
            {activeCenterView === "analytics" && <AnalyticsDashboard />}
            {activeCenterView === "trace" && <TraceabilityView />}
            {activeCenterView === "overview" && <JitOverview />}
            {activeCenterView === "map" && <FeatureCaseMap />}
          </Suspense>
        </div>
      ) : activeFile ? (
        <div className="flex flex-1 min-h-0">
          {isSplit ? (
            <div className="jit-editor-workbench flex flex-1 min-h-0 min-w-0 flex-row">
              <div style={{ flex: `${splitRatio} 1 0%` }} className="min-w-0 flex flex-col min-h-0">
                <div className="panel-header-bar flex items-center justify-between border-b border-border bg-[var(--surface-elevated)] shrink-0">
                  <span className="text-xs text-[var(--text)] font-mono truncate">{activeFile.path}</span>
                  <div className="flex gap-[var(--toolbar-gap)] shrink-0">
                    {isModelica && (
                      <button type="button" onClick={handleRunTest} disabled={running}
                        className="px-2 py-0.5 text-xs rounded border theme-banner-success disabled:opacity-50">
                        {running ? t("running") : t("runTest")}
                      </button>
                    )}
                    <button type="button" onClick={handleSave} disabled={!activeFile.dirty || saving}
                      className="px-2 py-0.5 text-xs rounded bg-primary hover:bg-blue-600 disabled:opacity-40">
                      {t("saveFile")}
                    </button>
                    <button type="button" onClick={handleRevert} disabled={!activeFile.dirty}
                      className="px-2 py-0.5 text-xs rounded border theme-button-secondary disabled:opacity-40">
                      {t("revertFile")}
                    </button>
                    {isRust && (
                      <button type="button" onClick={() => setShowSymbols(!showSymbols)}
                        className={`px-2 py-0.5 text-xs rounded border ${showSymbols ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary text-[var(--text-muted)]"}`}>
                        Symbols ({symbols.length})
                      </button>
                    )}
                  </div>
                </div>
                <div className="flex-1 min-h-0">
                  <Editor
                    height="100%"
                    language={isRust ? "rust" : "modelica"}
                    value={diffOverlay ?? activeFile.content}
                    onChange={diffOverlay ? undefined : (v) => onFileContentChange(activeFile.path, v ?? "")}
                    theme={theme === "light" ? "vs-light" : "vs-dark"}
                    options={{
                      readOnly: !!diffOverlay,
                      minimap: { enabled: false },
                      scrollBeyondLastLine: false,
                      fontSize: 13,
                    }}
                    onMount={(editorInstance, monacoInstance) => {
                      editorRef.current = editorInstance;
                      monacoRef.current = monacoInstance;
                      setEditorReady(true);
                      editorInstance.onDidFocusEditorText(() => setFocusedPane(0));
                      if (onSelectionChange) {
                        editorInstance.onDidChangeCursorSelection(() => {
                          const model = editorInstance.getModel();
                          if (!model) {
                            onSelectionChange({ path: activeFilePath, text: null });
                            return;
                          }
                          const selection = editorInstance.getSelection();
                          if (!selection) {
                            onSelectionChange({ path: activeFilePath, text: null });
                            return;
                          }
                          const selectedText = model.getValueInRange(selection);
                          onSelectionChange({ path: activeFilePath, text: selectedText || null });
                        });
                      }
                    }}
                  />
                </div>
              </div>
              <div
                className="editor-workbench-resize resize-handle shrink-0 flex-shrink-0 w-1 min-w-[4px]"
                onMouseDown={startResizeSplit}
                aria-hidden
              />
              <div style={{ flex: `${1 - splitRatio} 1 0%` }} className="min-w-0 flex flex-col min-h-0">
                <div className="panel-header-bar flex items-center border-b border-border bg-[var(--surface-elevated)] shrink-0">
                  <span className="text-xs text-[var(--text)] font-mono truncate">{activeFile.path}</span>
                </div>
                <div className="flex-1 min-h-0">
                  <Editor
                    height="100%"
                    language={isRust ? "rust" : "modelica"}
                    value={diffOverlay ?? activeFile.content}
                    onChange={diffOverlay ? undefined : (v) => onFileContentChange(activeFile.path, v ?? "")}
                    theme={theme === "light" ? "vs-light" : "vs-dark"}
                    options={{
                      readOnly: !!diffOverlay,
                      minimap: { enabled: false },
                      scrollBeyondLastLine: false,
                      fontSize: 13,
                    }}
                    onMount={(editorInstance) => {
                      editorRefSecondary.current = editorInstance;
                      editorInstance.onDidFocusEditorText(() => setFocusedPane(1));
                      if (onSelectionChange) {
                        editorInstance.onDidChangeCursorSelection(() => {
                          const model = editorInstance.getModel();
                          if (!model) {
                            onSelectionChange({ path: activeFilePath, text: null });
                            return;
                          }
                          const selection = editorInstance.getSelection();
                          if (!selection) {
                            onSelectionChange({ path: activeFilePath, text: null });
                            return;
                          }
                          const selectedText = model.getValueInRange(selection);
                          onSelectionChange({ path: activeFilePath, text: selectedText || null });
                        });
                      }
                    }}
                  />
                </div>
              </div>
            </div>
          ) : (
            <div className="flex-1 min-w-0 flex flex-col min-h-0">
              <div className="panel-header-bar flex items-center justify-between border-b border-border bg-[var(--surface-elevated)] shrink-0">
                <span className="text-xs text-[var(--text)] font-mono truncate">{activeFile.path}</span>
                <div className="flex gap-[var(--toolbar-gap)] shrink-0">
                  {isModelica && (
                    <button type="button" onClick={handleRunTest} disabled={running}
                      className="px-2 py-0.5 text-xs rounded border theme-banner-success disabled:opacity-50">
                      {running ? t("running") : t("runTest")}
                    </button>
                  )}
                  <button type="button" onClick={handleSave} disabled={!activeFile.dirty || saving}
                    className="px-2 py-0.5 text-xs rounded bg-primary hover:bg-blue-600 disabled:opacity-40">
                    {t("saveFile")}
                  </button>
                  <button type="button" onClick={handleRevert} disabled={!activeFile.dirty}
                    className="px-2 py-0.5 text-xs rounded border theme-button-secondary disabled:opacity-40">
                    {t("revertFile")}
                  </button>
                  {isRust && (
                    <button type="button" onClick={() => setShowSymbols(!showSymbols)}
                      className={`px-2 py-0.5 text-xs rounded border ${showSymbols ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary text-[var(--text-muted)]"}`}>
                      Symbols ({symbols.length})
                    </button>
                  )}
                </div>
              </div>
              <div className="flex-1 min-h-0">
                <Editor
                  height="100%"
                  language={isRust ? "rust" : "modelica"}
                  value={diffOverlay ?? activeFile.content}
                  onChange={diffOverlay ? undefined : (v) => onFileContentChange(activeFile.path, v ?? "")}
                  theme={theme === "light" ? "vs-light" : "vs-dark"}
                  options={{
                    readOnly: !!diffOverlay,
                    minimap: { enabled: false },
                    scrollBeyondLastLine: false,
                    fontSize: 13,
                  }}
                  onMount={(editorInstance, monacoInstance) => {
                    editorRef.current = editorInstance;
                    monacoRef.current = monacoInstance;
                    setEditorReady(true);
                    if (onSelectionChange) {
                      editorInstance.onDidChangeCursorSelection(() => {
                        const model = editorInstance.getModel();
                        if (!model) {
                          onSelectionChange({ path: activeFilePath, text: null });
                          return;
                        }
                        const selection = editorInstance.getSelection();
                        if (!selection) {
                          onSelectionChange({ path: activeFilePath, text: null });
                          return;
                        }
                        const selectedText = model.getValueInRange(selection);
                        onSelectionChange({ path: activeFilePath, text: selectedText || null });
                      });
                    }
                  }}
                />
              </div>
            </div>
          )}

          {showSymbols && isRust && (
            <div className="w-52 shrink-0 border-l border-border overflow-auto bg-[var(--panel-bg)]">
              <div className="panel-header-bar shrink-0 flex items-center border-b border-border">
                <span className="text-[10px] uppercase text-[var(--text-muted)]">Symbols ({symbols.length})</span>
              </div>
              {symbols.length > 0 ? (
                <div className="max-h-60 overflow-auto">
                  {symbols.map((s, i) => (
                    <button key={`${s.name}-${s.lineStart}-${i}`} type="button"
                      className="flex items-center gap-1 text-[11px] py-0.5 w-full text-left hover:bg-[var(--surface-hover)] rounded px-1"
                      title={s.signature || `${s.kind} ${s.name} (L${s.lineStart})`}>
                      <span className={`text-[9px] font-mono ${kindColor[s.kind] || "text-[var(--text)]"} w-4 shrink-0`}>
                        {s.kind.slice(0, 2).toUpperCase()}
                      </span>
                      <span className="text-[var(--text)] truncate">{s.name}</span>
                      <span className="text-[var(--text-muted)] text-[9px] ml-auto shrink-0">:{s.lineStart}</span>
                    </button>
                  ))}
                </div>
              ) : <div className="text-xs text-[var(--text-muted)] px-3 py-2">{t("none")}</div>}
              {gitLog.length > 0 && (
                <div className="panel-header-bar-tall shrink-0 flex flex-col gap-1 border-b border-border">
                  <div className="text-[10px] uppercase text-[var(--text-muted)]">{t("gitHistory")}</div>
                  {gitLog.map((g) => (
                    <div key={g.hash} className="mb-1.5">
                      <div className="text-[11px] text-[var(--text)] truncate" title={g.message}>{g.message}</div>
                      <div className="text-[10px] text-[var(--text-muted)]">{g.short} - {g.date}</div>
                    </div>
                  ))}
                </div>
              )}
              {moduleInfo && (
                <div className="panel-header-bar-tall shrink-0 flex flex-col gap-1 border-b border-border">
                  <div className="text-[10px] uppercase text-[var(--text-muted)]">{t("linkedFeatures")}</div>
                  {moduleInfo.features.length > 0 ? (
                    <div className="flex flex-wrap gap-1">
                      {moduleInfo.features.map((fid) => (
                        <span key={fid} className="px-1.5 py-0.5 rounded theme-banner-info text-[10px]">{fid}</span>
                      ))}
                    </div>
                  ) : <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>}
                </div>
              )}
              {linkedCases.length > 0 && (
                <div className="panel-header-bar-tall shrink-0 flex flex-col gap-1 border-b border-border">
                  <div className="text-[10px] uppercase text-[var(--text-muted)]">{t("linkedTests")}</div>
                  <div className="flex flex-wrap gap-1">
                    {linkedCases.map((c) => (
                      <span key={c} className="px-1.5 py-0.5 rounded theme-banner-success text-[10px]">{c.replace("TestLib/", "")}</span>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      ) : (
        <div className="flex-1 flex items-center justify-center text-sm text-[var(--text-muted)]">
          {t("noFileSelected")}
        </div>
      )}
    </div>
  );
});
