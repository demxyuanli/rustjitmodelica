import React, { useState, useCallback, useEffect, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import Editor from "@monaco-editor/react";
import { t } from "../i18n";
import { getSourceModules, getCases, type RegressionCase } from "../data/jit_regression_metadata";

interface IterationRecord {
  id: number;
  target: string;
  diff: string | null;
  success: boolean;
  message: string;
  created_at: string;
  branch_name?: string | null;
  duration_ms?: number | null;
}

interface MoRunDetail {
  name: string;
  expected: string;
  actual: string;
}

interface MoRunResult {
  passed: number;
  failed: number;
  details: MoRunDetail[];
}

interface RunResult {
  success: boolean;
  build_ok: boolean;
  test_ok: boolean;
  message: string;
  mo_run?: MoRunResult | null;
}

interface ChunkInfo {
  lineStart: number;
  lineEnd: number;
  content: string;
  contextLabel: string | null;
  contentHash: string;
  filePath: string;
}

interface SelfIterateUIProps {
  targetPrefill?: string | null;
  onClearPrefill?: () => void;
  fullScreen?: boolean;
  repoRoot?: string | null;
}

interface RoundState {
  round: number;
  target: string;
  contextFiles: string[];
  testCases: string[];
  diff: string | null;
  runResult: RunResult | null;
  editable: boolean;
}

const CARD_CLASS = "rounded-lg border border-gray-700 bg-[#2d2d2d] p-4 mb-4";

const STEP_LABELS = ["Select context", "Generate / Edit", "Test & Validate", "Adopt / Commit"];

function stepIndex(round: RoundState): number {
  if (round.runResult?.success && round.diff == null) return 4;
  if (round.runResult != null) return 3;
  if (round.diff != null) return 2;
  return 1;
}

export function SelfIterateUI({ targetPrefill, onClearPrefill, fullScreen, repoRoot: _repoRoot }: SelfIterateUIProps) {
  const [target, setTarget] = useState("");
  const [contextFiles, setContextFiles] = useState<string[]>([]);
  const [testCases, setTestCases] = useState<string[]>([]);
  const [diff, setDiff] = useState<string | null>(null);
  const [editableDiff, setEditableDiff] = useState(false);

  useEffect(() => {
    if (targetPrefill) {
      setTarget(targetPrefill);
      onClearPrefill?.();
    }
  }, [targetPrefill, onClearPrefill]);

  const [indexChunks, setIndexChunks] = useState<ChunkInfo[]>([]);
  const [indexContextLoading, setIndexContextLoading] = useState(false);

  const [patchLoading, setPatchLoading] = useState(false);
  const [runResult, setRunResult] = useState<RunResult | null>(null);
  const [runLoading, setRunLoading] = useState(false);
  const [adoptLoading, setAdoptLoading] = useState(false);
  const [commitLoading, setCommitLoading] = useState(false);
  const [history, setHistory] = useState<IterationRecord[]>([]);
  const [historyFilter, setHistoryFilter] = useState<"all" | "pass" | "fail">("all");
  const [expandedHistoryId, setExpandedHistoryId] = useState<number | null>(null);
  const [commitMessage, setCommitMessage] = useState("");
  const [banner, setBanner] = useState<{ message: string; type: "error" | "success" } | null>(null);
  const [round, setRound] = useState(1);
  const [showContextPicker, setShowContextPicker] = useState(false);
  const historySectionRef = useRef<HTMLDivElement>(null);

  const sourceModuleKeys = useMemo(() => Object.keys(getSourceModules()).sort(), []);
  const allCases: RegressionCase[] = useMemo(() => getCases(), []);

  const loadHistory = useCallback(async () => {
    try {
      const list = (await invoke("list_iteration_history", { limit: 50 })) as IterationRecord[];
      setHistory(list);
    } catch {}
  }, []);

  useEffect(() => { loadHistory(); }, [loadHistory]);

  useEffect(() => {
    if (banner?.type === "success") {
      const tm = setTimeout(() => setBanner(null), 4000);
      return () => clearTimeout(tm);
    }
  }, [banner]);

  const handleGeneratePatch = useCallback(async () => {
    if (!target.trim()) return;
    setPatchLoading(true);
    setDiff(null);
    setRunResult(null);
    setBanner(null);
    try {
      let result: string;
      if (contextFiles.length > 0 || testCases.length > 0) {
        result = await invoke<string>("ai_generate_compiler_patch_with_context", {
          target: target.trim(),
          contextFiles,
          testCases,
        });
      } else {
        result = await invoke<string>("ai_generate_compiler_patch", { target: target.trim() });
      }
      setDiff(result);
      setEditableDiff(false);
    } catch (e) {
      setDiff("Error: " + String(e));
    } finally {
      setPatchLoading(false);
    }
  }, [target, contextFiles, testCases]);

  const handleRunInSandbox = useCallback(async () => {
    setRunLoading(true);
    setRunResult(null);
    setBanner(null);
    try {
      const result = (await invoke("self_iterate", { diff: diff || undefined })) as RunResult;
      setRunResult(result);
    } catch (e) {
      setRunResult({ success: false, build_ok: false, test_ok: false, message: String(e) });
    } finally {
      setRunLoading(false);
    }
  }, [diff]);

  const handleAdoptToWorkspace = useCallback(async () => {
    if (diff == null) return;
    setAdoptLoading(true);
    setBanner(null);
    try {
      await invoke("apply_patch_to_workspace", { diff });
      setDiff(null);
      setBanner({ message: t("adoptedSuccess"), type: "success" });
    } catch (e) {
      setBanner({ message: String(e), type: "error" });
    } finally {
      setAdoptLoading(false);
    }
  }, [diff]);

  const handleCommitPatch = useCallback(async () => {
    setCommitLoading(true);
    setBanner(null);
    const msg = commitMessage.trim() || "Self-iteration patch";
    try {
      await invoke("commit_patch", { message: msg });
      setBanner({ message: t("committedSuccess"), type: "success" });
      loadHistory();
    } catch (e) {
      setBanner({ message: String(e), type: "error" });
    } finally {
      setCommitLoading(false);
    }
  }, [commitMessage, loadHistory]);

  const handleSaveToHistory = useCallback(async () => {
    if (!runResult) return;
    try {
      await invoke("save_iteration", {
        target: target.trim(),
        diff: diff || null,
        success: runResult.success,
        message: runResult.message,
      });
      loadHistory();
    } catch {}
  }, [target, diff, runResult, loadHistory]);

  const handleNextRound = useCallback(() => {
    const failedCases = runResult?.mo_run?.details.filter((d) => d.actual !== d.expected).map((d) => d.name) ?? [];
    const failMsg = runResult?.message ?? "";
    setRound((r) => r + 1);
    setDiff(null);
    setRunResult(null);
    if (failedCases.length > 0) {
      setTarget((prev) => `${prev}\n\n[Round ${round} failure] ${failMsg}\nFailed cases: ${failedCases.join(", ")}`);
    }
  }, [runResult, round]);

  const handleExportHistory = useCallback(() => {
    const list = historyFilter === "all" ? history : history.filter((r) => (historyFilter === "pass" ? r.success : !r.success));
    const headers = "ID,Target,Success,Message,Date\n";
    const rows = list.map((r) => `${r.id},"${(r.target || "").replace(/"/g, '""')}",${r.success},"${(r.message || "").replace(/"/g, '""')}",${r.created_at}`).join("\n");
    const blob = new Blob(["\ufeff" + headers + rows], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `iteration-history-${new Date().toISOString().slice(0, 10)}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }, [history, historyFilter]);

  const toggleContextFile = useCallback((path: string) => {
    setContextFiles((prev) => prev.includes(path) ? prev.filter((p) => p !== path) : [...prev, path]);
  }, []);

  const toggleTestCase = useCallback((name: string) => {
    setTestCases((prev) => prev.includes(name) ? prev.filter((n) => n !== name) : [...prev, name]);
  }, []);

  const currentRound: RoundState = { round, target, contextFiles, testCases, diff, runResult, editable: editableDiff };
  const currentStep = stepIndex(currentRound);

  const filteredHistory = historyFilter === "all" ? history : history.filter((r) => (historyFilter === "pass" ? r.success : !r.success));
  const recentHistory = history.slice(0, 3);
  const rootClass = fullScreen ? "flex flex-col h-full min-h-0 overflow-auto p-4" : "mt-3 pt-3 border-t border-border p-4";

  return (
    <div className={rootClass}>
      {banner && (
        <div className={`mb-4 rounded-lg border px-4 py-3 flex items-center justify-between shrink-0 ${
          banner.type === "error" ? "bg-red-900/30 border-red-700 text-red-200" : "bg-green-900/30 border-green-700 text-green-200"
        }`}>
          <span className="text-sm">{banner.message}</span>
          <button type="button" className="ml-2 text-current opacity-80 hover:opacity-100" onClick={() => setBanner(null)}>&#215;</button>
        </div>
      )}

      {/* Round indicator */}
      <div className="flex items-center gap-3 mb-4 shrink-0">
        <span className="text-xs font-medium text-primary bg-primary/15 px-3 py-1 rounded-lg">
          {t("iterationRound")} {round}
        </span>
        {history.length > 0 && (
          <div className="flex gap-1">
            {recentHistory.map((r) => (
              <button
                key={r.id}
                type="button"
                className="px-2 py-0.5 text-[10px] rounded border border-gray-600 bg-[#3c3c3c] hover:bg-gray-600"
                onClick={() => { setExpandedHistoryId(r.id); historySectionRef.current?.scrollIntoView({ behavior: "smooth" }); }}
              >
                <span className={r.success ? "text-green-400" : "text-red-400"}>#{r.id}</span>
              </button>
            ))}
          </div>
        )}
      </div>

      <nav className="flex items-center gap-2 mb-4 shrink-0 flex-wrap">
        {STEP_LABELS.map((label, i) => (
          <span key={i} className={`text-xs font-medium ${currentStep >= i + 1 ? "text-primary" : "text-[var(--text-muted)]"}`}>
            {i + 1}. {label}
          </span>
        ))}
      </nav>

      {/* Step 1: Target + Context */}
      <section className={CARD_CLASS}>
        <h4 className="text-sm font-medium text-[var(--text)] mb-2">1. {t("selectContext")}</h4>
        <label className="text-xs text-[var(--text-muted)] block mb-1">{t("selfIterateTarget")}</label>
        <textarea
          value={target}
          onChange={(e) => setTarget(e.target.value)}
          placeholder="e.g. Add sparse Jacobian support"
          className="w-full bg-[#3c3c3c] border border-gray-600 px-3 py-2 text-sm resize-none rounded-lg h-20"
        />

        {/* Context files toggle */}
        <div className="mt-3">
          <button type="button" onClick={() => setShowContextPicker(!showContextPicker)} className="text-xs text-primary hover:underline">
            {t("contextFiles")} ({contextFiles.length}) / {t("testCasesContext")} ({testCases.length})
            {showContextPicker ? " \u25B2" : " \u25BC"}
          </button>
        </div>

        {showContextPicker && (
          <div className="mt-2 grid grid-cols-2 gap-3">
            <div className="rounded-lg border border-gray-700 bg-[#1e1e1e] max-h-40 overflow-auto p-2">
              <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("contextFiles")}</div>
              {sourceModuleKeys.map((path) => (
                <label key={path} className="flex items-center gap-1 text-[11px] py-0.5 cursor-pointer hover:bg-[#3c3c3c]/30">
                  <input type="checkbox" checked={contextFiles.includes(path)} onChange={() => toggleContextFile(path)} className="w-3 h-3" />
                  <span className="text-[var(--text)] truncate font-mono">{path.replace("src/", "")}</span>
                </label>
              ))}
            </div>
            <div className="rounded-lg border border-gray-700 bg-[#1e1e1e] max-h-40 overflow-auto p-2">
              <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("testCasesContext")}</div>
              {allCases.map((c) => (
                <label key={c.name} className="flex items-center gap-1 text-[11px] py-0.5 cursor-pointer hover:bg-[#3c3c3c]/30">
                  <input type="checkbox" checked={testCases.includes(c.name)} onChange={() => toggleTestCase(c.name)} className="w-3 h-3" />
                  <span className="text-[var(--text)] truncate">{c.name.replace("TestLib/", "")}</span>
                </label>
              ))}
            </div>
          </div>
        )}

        <div className="mt-3 flex gap-2 items-center flex-wrap">
          <button type="button" onClick={handleGeneratePatch} disabled={patchLoading}
            className="px-4 py-2 bg-primary hover:bg-blue-600 text-sm font-medium rounded-lg disabled:opacity-50">
            {patchLoading ? t("running") : t("generatePatch")}
          </button>
          <button
            type="button"
            disabled={!target.trim() || indexContextLoading}
            onClick={async () => {
              if (!target.trim()) return;
              setIndexContextLoading(true);
              try {
                const chunks = await invoke<ChunkInfo[]>("index_repo_get_context", {
                  query: target.trim(),
                  maxChunks: 8,
                });
                setIndexChunks(chunks);
                const paths = [...new Set(chunks.map((c) => c.filePath))];
                setContextFiles((prev) => {
                  const merged = [...prev];
                  for (const p of paths) {
                    if (!merged.includes(p)) merged.push(p);
                  }
                  return merged;
                });
              } catch {
                setIndexChunks([]);
              } finally {
                setIndexContextLoading(false);
              }
            }}
            className="px-4 py-2 bg-[#3c3c3c] hover:bg-gray-600 text-sm font-medium rounded-lg border border-gray-600 disabled:opacity-50"
          >
            {indexContextLoading ? t("running") : "Smart Context"}
          </button>
          {indexChunks.length > 0 && (
            <span className="text-xs text-[var(--text-muted)]">
              {indexChunks.length} chunks from {[...new Set(indexChunks.map((c) => c.filePath))].length} files
            </span>
          )}
        </div>
        {indexChunks.length > 0 && (
          <div className="mt-2 rounded-lg border border-gray-700 bg-[#1e1e1e] max-h-32 overflow-auto p-2">
            <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">Index Context</div>
            {indexChunks.map((chunk, i) => (
              <div key={i} className="text-[11px] py-0.5 border-b border-gray-700/50 last:border-b-0">
                <span className="text-primary font-mono">{chunk.filePath}</span>
                <span className="text-[var(--text-muted)]"> L{chunk.lineStart}-{chunk.lineEnd}</span>
                {chunk.contextLabel && (
                  <span className="text-[var(--text-muted)] ml-1">({chunk.contextLabel})</span>
                )}
              </div>
            ))}
          </div>
        )}
      </section>

      {/* Step 2: Diff & Edit */}
      {diff != null ? (
        <section className={CARD_CLASS}>
          <div className="flex items-center justify-between mb-2">
            <h4 className="text-sm font-medium text-[var(--text)]">2. {t("stepDiff")}</h4>
            <button type="button" onClick={() => setEditableDiff(!editableDiff)}
              className={`text-xs px-2 py-0.5 rounded ${editableDiff ? "bg-amber-800 text-amber-200" : "bg-[#3c3c3c] text-[var(--text-muted)]"}`}>
              {editableDiff ? "Read-only" : "Edit diff"}
            </button>
          </div>
          <div className="rounded-lg border border-gray-700 overflow-hidden h-56">
            <Editor
              height="100%"
              language="plaintext"
              value={diff}
              onChange={editableDiff ? (v) => setDiff(v ?? "") : undefined}
              options={{ readOnly: !editableDiff, wordWrap: "on", minimap: { enabled: false }, lineNumbers: "on", scrollBeyondLastLine: false }}
            />
          </div>
          <div className="flex gap-2 mt-3">
            <button type="button" onClick={handleRunInSandbox} disabled={runLoading}
              className="px-4 py-2 bg-primary hover:bg-blue-600 text-sm font-medium rounded-lg disabled:opacity-50">
              {runLoading ? t("running") : t("runInSandbox")}
            </button>
            {runResult != null && (
              <button type="button" onClick={handleSaveToHistory} className="px-4 py-2 bg-[#3c3c3c] hover:bg-gray-600 text-sm rounded-lg border border-gray-600">
                {t("saveToHistory")}
              </button>
            )}
          </div>
        </section>
      ) : (
        <section className={`${CARD_CLASS} opacity-80`}>
          <h4 className="text-sm font-medium text-[var(--text-muted)]">2. {t("stepDiff")}</h4>
          <p className="text-xs text-[var(--text-muted)]">{t("completePreviousStep")}</p>
        </section>
      )}

      {/* Step 3: Results */}
      {runResult != null ? (
        <section className={CARD_CLASS}>
          <h4 className="text-sm font-medium text-[var(--text)] mb-2">3. {t("stepMoResult")}</h4>
          <div className={`text-sm p-3 rounded-lg mb-3 ${runResult.success ? "bg-green-900/30 text-green-300" : "bg-red-900/30 text-red-300"}`}>
            {runResult.message}
          </div>
          {runResult.mo_run != null && runResult.mo_run.details.length > 0 && (
            <div className="overflow-hidden rounded-lg border border-gray-700">
              <div className="text-xs text-[var(--text-muted)] px-3 py-2 border-b border-gray-700 bg-[#3c3c3c]">
                {runResult.mo_run.failed === 0
                  ? t("moCasesPassed").replace("{passed}", String(runResult.mo_run.passed))
                  : t("stepSummaryMo").replace("{n}", String(runResult.mo_run.passed)) + ", " + runResult.mo_run.failed + " failed"}
              </div>
              <div className="overflow-auto max-h-48">
                <table className="w-full text-xs">
                  <thead>
                    <tr className="text-left text-[var(--text-muted)] border-b border-gray-700 bg-[#3c3c3c]">
                      <th className="px-3 py-1.5 font-medium">Case</th>
                      <th className="px-3 py-1.5 font-medium w-20">Expected</th>
                      <th className="px-3 py-1.5 font-medium w-20">Actual</th>
                      <th className="px-3 py-1.5 font-medium w-16">Status</th>
                    </tr>
                  </thead>
                  <tbody>
                    {runResult.mo_run.details.map((d) => {
                      const ok = d.actual === d.expected;
                      return (
                        <tr key={d.name} className={`border-b border-gray-700/50 ${ok ? "" : "bg-red-900/20"}`}>
                          <td className="px-3 py-1.5 text-[var(--text)]">{d.name}</td>
                          <td className="px-3 py-1.5">{d.expected}</td>
                          <td className="px-3 py-1.5">{d.actual}</td>
                          <td className="px-3 py-1.5">{ok ? <span className="text-green-400">OK</span> : <span className="text-red-400">Fail</span>}</td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </div>
          )}
          {!runResult.success && (
            <div className="mt-3">
              <button type="button" onClick={handleNextRound}
                className="px-4 py-2 bg-amber-700 hover:bg-amber-600 text-sm font-medium rounded-lg">
                {t("analyzeFailures")} &rarr; Round {round + 1}
              </button>
            </div>
          )}
        </section>
      ) : (
        diff != null && (
          <section className={`${CARD_CLASS} opacity-80`}>
            <h4 className="text-sm font-medium text-[var(--text-muted)]">3. {t("stepMoResult")}</h4>
            <p className="text-xs text-[var(--text-muted)]">{t("completePreviousStep")}</p>
          </section>
        )
      )}

      {/* Step 4: Adopt */}
      <section className={CARD_CLASS}>
        <h4 className="text-sm font-medium text-[var(--text)] mb-2">4. {t("stepAdopt")}</h4>
        {runResult?.success ? (
          <div className="space-y-3">
            {diff != null && (
              <button type="button" onClick={handleAdoptToWorkspace} disabled={adoptLoading}
                className="px-4 py-2 bg-green-700 hover:bg-green-600 text-sm font-medium rounded-lg disabled:opacity-50">
                {adoptLoading ? t("running") : t("adoptToWorkspace")}
              </button>
            )}
            {diff == null && (
              <>
                <label className="text-xs text-[var(--text-muted)] block mb-1">Commit message</label>
                <input type="text" value={commitMessage} onChange={(e) => setCommitMessage(e.target.value)}
                  placeholder={t("commitMessagePlaceholder")}
                  className="w-full max-w-md bg-[#3c3c3c] border border-gray-600 px-3 py-2 text-sm rounded-lg" />
                <div className="mt-2">
                  <button type="button" onClick={handleCommitPatch} disabled={commitLoading}
                    className="px-4 py-2 bg-[#3c3c3c] hover:bg-gray-600 text-sm font-medium rounded-lg border border-gray-600 disabled:opacity-50">
                    {commitLoading ? t("running") : t("commitPatch")}
                  </button>
                </div>
              </>
            )}
          </div>
        ) : (
          <p className="text-xs text-[var(--text-muted)]">{t("completePreviousStep")}</p>
        )}
      </section>

      {/* History */}
      <section ref={historySectionRef} className={fullScreen ? "flex-1 min-h-0 flex flex-col overflow-hidden mt-4" : "mt-4"}>
        <div className="flex items-center justify-between mb-2 shrink-0">
          <span className="text-sm font-medium text-[var(--text-muted)]">{t("iterationHistory")}</span>
          <div className="flex gap-2 items-center">
            {(["all", "pass", "fail"] as const).map((f) => (
              <button key={f} type="button"
                className={`px-3 py-1 text-xs rounded-lg ${historyFilter === f ? "bg-primary/30 text-primary" : "bg-[#3c3c3c] text-[var(--text-muted)] hover:text-[var(--text)]"}`}
                onClick={() => setHistoryFilter(f)}>
                {f === "all" ? "All" : f === "pass" ? "Pass" : "Fail"}
              </button>
            ))}
            {filteredHistory.length > 0 && (
              <button type="button" onClick={handleExportHistory} className="px-3 py-1 text-xs rounded-lg bg-[#3c3c3c] hover:bg-gray-600 text-[var(--text-muted)] hover:text-[var(--text)]">
                {t("exportHistory")}
              </button>
            )}
          </div>
        </div>
        <div className={`rounded-lg border border-gray-700 bg-[#2d2d2d] overflow-auto ${fullScreen ? "flex-1 min-h-0" : "max-h-48"}`}>
          {filteredHistory.length === 0 ? (
            <div className="px-4 py-8 text-sm text-[var(--text-muted)] text-center">{t("noHistoryYet")}</div>
          ) : (
            <table className="w-full text-xs">
              <thead className="sticky top-0 bg-[#2d2d2d] z-10">
                <tr className="text-left text-[var(--text-muted)] border-b border-gray-700">
                  <th className="px-3 py-2 font-medium w-12">ID</th>
                  <th className="px-3 py-2 font-medium">Target</th>
                  <th className="px-3 py-2 font-medium w-16">Success</th>
                  <th className="px-3 py-2 font-medium w-36">Date</th>
                  <th className="px-3 py-2 font-medium w-16"></th>
                </tr>
              </thead>
              <tbody>
                {filteredHistory.map((r) => (
                  <React.Fragment key={r.id}>
                    <tr className={`border-b border-gray-700/50 hover:bg-[#3c3c3c]/30 cursor-pointer ${expandedHistoryId === r.id ? "bg-[#3c3c3c]/30" : ""}`}
                      onClick={() => setExpandedHistoryId((prev) => (prev === r.id ? null : r.id))}>
                      <td className="px-3 py-2 font-mono text-[var(--text)]">{r.id}</td>
                      <td className="px-3 py-2 text-[var(--text)] truncate max-w-[200px]" title={r.target}>{r.target || "\u2014"}</td>
                      <td className="px-3 py-2"><span className={r.success ? "text-green-400" : "text-red-400"}>{r.success ? "Yes" : "No"}</span></td>
                      <td className="px-3 py-2 text-[var(--text-muted)]">{r.created_at.slice(0, 19)}</td>
                      <td className="px-3 py-2">{expandedHistoryId === r.id ? "\u25BC" : "\u25B6"}</td>
                    </tr>
                    {expandedHistoryId === r.id && (
                      <tr className="border-b border-gray-700/50 bg-[#252526]">
                        <td colSpan={5} className="px-3 py-2 text-[var(--text-muted)] text-xs whitespace-pre-wrap">{r.message || "\u2014"}</td>
                      </tr>
                    )}
                  </React.Fragment>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </section>
    </div>
  );
}
