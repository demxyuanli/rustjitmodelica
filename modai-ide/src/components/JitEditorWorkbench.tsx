import React, { useState, useEffect, useCallback, useRef, forwardRef, useImperativeHandle, lazy, Suspense } from "react";
import { invoke } from "@tauri-apps/api/core";
import Editor from "@monaco-editor/react";
import type monaco from "monaco-editor";
import { t } from "../i18n";
import { getSourceModules, getCaseToSourceFiles } from "../data/jit_regression_metadata";
import type { JitCenterView } from "../hooks/useJitLayout";
import { useEditorDiffDecorations } from "../hooks/useEditorDiffDecorations";
import { SettingsContent, type SettingsContentProps } from "./SettingsContent";
import { AppIcon } from "./Icon";

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
  settings:  { labelKey: "settings", icon: <AppIcon name="settings" aria-hidden="true" /> },
};

export type SettingsViewProps = SettingsContentProps;

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
  settingsProps?: SettingsViewProps;
  onSelectionChange?: (params: { path: string | null; text: string | null }) => void;
  repoRoot?: string | null;
}

function SettingsInlineView(props: SettingsViewProps) {
  return (
    <div className="max-w-2xl mx-auto p-6 text-[var(--text)]">
      <h2 className="text-lg font-semibold text-[var(--text)] mb-6">{t("settings")}</h2>
      <SettingsContent {...props} />
    </div>
  );
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
    settingsProps,
    onSelectionChange,
    repoRoot,
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

  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<typeof monaco | null>(null);

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
        if (!editorRef.current || !text || diffOverlay) return;
        const editor = editorRef.current;
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
    [diffOverlay]
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
        <div className={`px-4 py-1.5 text-xs shrink-0 ${banner.type === "error" ? "bg-red-900/30 text-red-300" : "bg-green-900/30 text-green-300"}`}>
          {banner.msg}
        </div>
      )}

      {/* Toolbar: analysis view buttons */}
      <div className="flex items-center gap-1 px-2 py-1 border-b border-gray-700 bg-[#2d2d2d] shrink-0">
        <span className="text-[10px] text-[var(--text-muted)] mr-1">{t("view")}:</span>
        {(Object.keys(CENTER_VIEW_META) as JitCenterView[]).map((viewId) => {
          const meta = CENTER_VIEW_META[viewId];
          const isActive = activeCenterView === viewId;
          return (
            <button key={viewId} type="button"
              className={`px-2 py-0.5 text-xs rounded ${isActive ? "bg-primary text-white" : "bg-[#3c3c3c] text-[var(--text-muted)] hover:bg-gray-600 hover:text-[var(--text)]"}`}
              onClick={() => onCenterViewChange(isActive ? null : viewId)}
              title={t(meta.labelKey as Parameters<typeof t>[0])}>
              <span className="inline-flex items-center gap-1">
                {meta.icon}
                <span>{t(meta.labelKey as Parameters<typeof t>[0])}</span>
              </span>
            </button>
          );
        })}
      </div>

      {/* Tab bar: file tabs + active center view tab */}
      <div className="flex border-b border-gray-700 shrink-0 bg-[#2d2d2d] overflow-x-auto">
        {openFiles.map((f) => {
          const isActive = showingEditor && f.path === activeFilePath;
          const label = f.type === "modelica" ? f.path.replace("TestLib/", "") : f.path.replace("src/", "");
          return (
            <div key={f.path}
              className={`flex items-center gap-1 px-3 py-1.5 text-xs cursor-pointer border-r border-gray-700 ${
                isActive ? "bg-[#1e1e1e] text-[var(--text)]" : "text-[var(--text-muted)] hover:bg-[#3c3c3c]"
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
          <div className="flex items-center gap-1 px-3 py-1.5 text-xs cursor-pointer border-r border-gray-700 bg-[#1e1e1e] text-[var(--text)]">
            <span className="text-primary font-medium">
              {t(CENTER_VIEW_META[activeCenterView].labelKey as Parameters<typeof t>[0])}
            </span>
            <button type="button" className="ml-1 text-[var(--text-muted)] hover:text-[var(--text)] text-[10px]"
              onClick={() => onCenterViewChange(null)}
              title={t("closeTab")}>&#215;</button>
          </div>
        )}
      </div>

      {/* Main content: editor or analysis/settings view */}
      {activeCenterView ? (
        <div className="flex-1 min-h-0 overflow-auto">
          {activeCenterView === "settings" && settingsProps ? (
            <SettingsInlineView {...settingsProps} />
          ) : (
            <Suspense fallback={<div className="p-4 text-[var(--text-muted)] text-xs">{t("loading")}</div>}>
              {activeCenterView === "analytics" && <AnalyticsDashboard />}
              {activeCenterView === "trace" && <TraceabilityView />}
              {activeCenterView === "overview" && <JitOverview />}
              {activeCenterView === "map" && <FeatureCaseMap />}
            </Suspense>
          )}
        </div>
      ) : activeFile ? (
        <div className="flex flex-1 min-h-0">
          <div className="flex-1 min-w-0 flex flex-col min-h-0">
            <div className="flex items-center justify-between px-3 py-1 border-b border-gray-700 bg-[#2d2d2d] shrink-0">
              <span className="text-xs text-[var(--text)] font-mono truncate">{activeFile.path}</span>
              <div className="flex gap-2 shrink-0">
                {isModelica && (
                  <button type="button" onClick={handleRunTest} disabled={running}
                    className="px-2 py-0.5 text-xs rounded bg-green-700 hover:bg-green-600 disabled:opacity-50">
                    {running ? t("running") : t("runTest")}
                  </button>
                )}
                <button type="button" onClick={handleSave} disabled={!activeFile.dirty || saving}
                  className="px-2 py-0.5 text-xs rounded bg-primary hover:bg-blue-600 disabled:opacity-40">
                  {t("saveFile")}
                </button>
                <button type="button" onClick={handleRevert} disabled={!activeFile.dirty}
                  className="px-2 py-0.5 text-xs rounded bg-[#3c3c3c] hover:bg-gray-600 disabled:opacity-40">
                  {t("revertFile")}
                </button>
                {isRust && (
                  <button type="button" onClick={() => setShowSymbols(!showSymbols)}
                    className={`px-2 py-0.5 text-xs rounded ${showSymbols ? "bg-primary/30 text-primary" : "bg-[#3c3c3c] text-[var(--text-muted)]"}`}>
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
                theme="vs-dark"
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

          {showSymbols && isRust && (
            <div className="w-52 shrink-0 border-l border-gray-700 overflow-auto bg-[#252526]">
              <div className="px-3 py-2 border-b border-gray-700">
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">Symbols ({symbols.length})</div>
                {symbols.length > 0 ? (
                  <div className="max-h-60 overflow-auto">
                    {symbols.map((s, i) => (
                      <button key={`${s.name}-${s.lineStart}-${i}`} type="button"
                        className="flex items-center gap-1 text-[11px] py-0.5 w-full text-left hover:bg-white/10 rounded px-1"
                        title={s.signature || `${s.kind} ${s.name} (L${s.lineStart})`}>
                        <span className={`text-[9px] font-mono ${kindColor[s.kind] || "text-[var(--text)]"} w-4 shrink-0`}>
                          {s.kind.slice(0, 2).toUpperCase()}
                        </span>
                        <span className="text-[var(--text)] truncate">{s.name}</span>
                        <span className="text-[var(--text-muted)] text-[9px] ml-auto shrink-0">:{s.lineStart}</span>
                      </button>
                    ))}
                  </div>
                ) : <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>}
              </div>
              {gitLog.length > 0 && (
                <div className="px-3 py-2 border-b border-gray-700">
                  <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("gitHistory")}</div>
                  {gitLog.map((g) => (
                    <div key={g.hash} className="mb-1.5">
                      <div className="text-[11px] text-[var(--text)] truncate" title={g.message}>{g.message}</div>
                      <div className="text-[10px] text-[var(--text-muted)]">{g.short} - {g.date}</div>
                    </div>
                  ))}
                </div>
              )}
              {moduleInfo && (
                <div className="px-3 py-2 border-b border-gray-700">
                  <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("linkedFeatures")}</div>
                  {moduleInfo.features.length > 0 ? (
                    <div className="flex flex-wrap gap-1">
                      {moduleInfo.features.map((fid) => (
                        <span key={fid} className="px-1.5 py-0.5 rounded bg-blue-900/40 text-blue-300 text-[10px]">{fid}</span>
                      ))}
                    </div>
                  ) : <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>}
                </div>
              )}
              {linkedCases.length > 0 && (
                <div className="px-3 py-2 border-b border-gray-700">
                  <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("linkedTests")}</div>
                  <div className="flex flex-wrap gap-1">
                    {linkedCases.map((c) => (
                      <span key={c} className="px-1.5 py-0.5 rounded bg-green-900/40 text-green-300 text-[10px]">{c.replace("TestLib/", "")}</span>
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
