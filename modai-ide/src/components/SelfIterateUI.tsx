import React, { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import Editor from "@monaco-editor/react";
import { t } from "../i18n";

interface IterationRecord {
  id: number;
  target: string;
  diff: string | null;
  success: boolean;
  message: string;
  created_at: string;
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

interface SelfIterateUIProps {
  targetPrefill?: string | null;
  onClearPrefill?: () => void;
  fullScreen?: boolean;
}

function stepIndex(diff: string | null, runResult: RunResult | null): number {
  if (runResult?.success && diff == null) return 4;
  if (runResult != null) return 3;
  if (diff != null) return 2;
  return 1;
}

const CARD_CLASS = "rounded-lg border border-gray-700 bg-[#2d2d2d] p-4 mb-4";

export function SelfIterateUI({ targetPrefill, onClearPrefill, fullScreen }: SelfIterateUIProps) {
  const [target, setTarget] = useState("");
  const [diff, setDiff] = useState<string | null>(null);
  useEffect(() => {
    if (targetPrefill) {
      setTarget(targetPrefill);
      onClearPrefill?.();
    }
  }, [targetPrefill, onClearPrefill]);
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
  const historySectionRef = useRef<HTMLDivElement>(null);

  const loadHistory = useCallback(async () => {
    try {
      const list = (await invoke("list_iteration_history", { limit: 50 })) as IterationRecord[];
      setHistory(list);
    } catch {}
  }, []);

  useEffect(() => {
    loadHistory();
  }, [loadHistory]);

  useEffect(() => {
    if (banner?.type === "success") {
      const t = setTimeout(() => setBanner(null), 4000);
      return () => clearTimeout(t);
    }
  }, [banner]);

  const handleGeneratePatch = useCallback(async () => {
    if (!target.trim()) return;
    setPatchLoading(true);
    setDiff(null);
    setRunResult(null);
    setBanner(null);
    try {
      const result = (await invoke("ai_generate_compiler_patch", { target: target.trim() })) as string;
      setDiff(result);
    } catch (e) {
      setDiff("Error: " + String(e));
    } finally {
      setPatchLoading(false);
    }
  }, [target]);

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
    } catch (e) {
      setBanner({ message: String(e), type: "error" });
    } finally {
      setCommitLoading(false);
    }
  }, [commitMessage]);

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

  const currentStep = stepIndex(diff, runResult);
  const filteredHistory =
    historyFilter === "all"
      ? history
      : history.filter((r) => (historyFilter === "pass" ? r.success : !r.success));
  const recentHistory = history.slice(0, 3);

  const rootClass = fullScreen ? "flex flex-col h-full min-h-0 overflow-auto p-4" : "mt-3 pt-3 border-t border-border p-4";

  return (
    <div className={rootClass}>
      {banner && (
        <div
          className={`mb-4 rounded-lg border px-4 py-3 flex items-center justify-between shrink-0 ${
            banner.type === "error" ? "bg-red-900/30 border-red-700 text-red-200" : "bg-green-900/30 border-green-700 text-green-200"
          }`}
        >
          <span className="text-sm">{banner.message}</span>
          <button type="button" className="ml-2 text-current opacity-80 hover:opacity-100" onClick={() => setBanner(null)} aria-label="Close">
            &#215;
          </button>
        </div>
      )}

      {history.length > 0 && (
        <section className={`${CARD_CLASS} shrink-0`}>
          <h3 className="text-sm font-medium text-[var(--text)] mb-2">{t("lastRun")}</h3>
          <div className="flex flex-wrap gap-2">
            {recentHistory.map((r) => (
              <button
                key={r.id}
                type="button"
                className="text-left px-3 py-2 rounded-lg border border-gray-600 bg-[#3c3c3c] hover:bg-gray-600 text-xs max-w-[280px] truncate"
                onClick={() => {
                  setExpandedHistoryId((prev) => (prev === r.id ? null : r.id));
                  historySectionRef.current?.scrollIntoView({ behavior: "smooth" });
                }}
              >
                <span className={r.success ? "text-green-400" : "text-red-400"}>#{r.id}</span>{" "}
                {(r.target || "\u2014").slice(0, 20)}
                {r.target && r.target.length > 20 ? "..." : ""} {r.success ? "OK" : "Fail"}
              </button>
            ))}
          </div>
        </section>
      )}

      <nav className="flex items-center gap-2 mb-4 shrink-0 flex-wrap" aria-label="Steps">
        {[1, 2, 3, 4].map((step) => (
          <span
            key={step}
            aria-current={currentStep === step ? "step" : undefined}
            className={`text-xs font-medium ${currentStep >= step ? "text-primary" : "text-[var(--text-muted)]"}`}
          >
            {step}. {step === 1 ? t("stepTarget") : step === 2 ? t("stepDiff") : step === 3 ? t("stepMoResult") : t("stepAdopt")}
          </span>
        ))}
      </nav>

      <section className={CARD_CLASS} aria-label="Step 1">
        <h4 className="text-sm font-medium text-[var(--text)] mb-2">1. {t("stepTarget")}</h4>
        <label className="text-xs text-[var(--text-muted)] block mb-1">{t("selfIterateTarget")}</label>
        <textarea
          value={target}
          onChange={(e) => setTarget(e.target.value)}
          placeholder="e.g. Add sparse Jacobian support"
          className="w-full bg-[#3c3c3c] border border-gray-600 px-3 py-2 text-sm resize-none rounded-lg h-20"
        />
        <div className="mt-2">
          <button
            type="button"
            onClick={handleGeneratePatch}
            disabled={patchLoading}
            className="px-4 py-2 bg-primary hover:bg-blue-600 text-sm font-medium rounded-lg disabled:opacity-50"
          >
            {patchLoading ? t("running") : t("generatePatch")}
          </button>
        </div>
      </section>

      {diff != null ? (
        <section className={CARD_CLASS} aria-label="Step 2">
          <h4 className="text-sm font-medium text-[var(--text)] mb-2">2. {t("stepDiff")}</h4>
          <p className="text-xs text-[var(--text-muted)] mb-2">{t("stepSummaryDiff")}</p>
          <div className="rounded-lg border border-gray-700 overflow-hidden h-56">
            <Editor
              height="100%"
              language="plaintext"
              value={diff}
              options={{ readOnly: true, wordWrap: "on", minimap: { enabled: false }, lineNumbers: "on", scrollBeyondLastLine: false }}
            />
          </div>
          <div className="flex gap-2 mt-3">
            <button
              type="button"
              onClick={handleRunInSandbox}
              disabled={runLoading}
              className="px-4 py-2 bg-primary hover:bg-blue-600 text-sm font-medium rounded-lg disabled:opacity-50"
            >
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

      {runResult != null ? (
        <section className={CARD_CLASS} aria-label="Step 3">
          <h4 className="text-sm font-medium text-[var(--text)] mb-2">3. {t("stepMoResult")}</h4>
          <div
            className={`text-sm p-3 rounded-lg mb-3 ${runResult.success ? "bg-green-900/30 text-green-300" : "bg-red-900/30 text-red-300"}`}
          >
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
        </section>
      ) : (
        diff != null && (
          <section className={`${CARD_CLASS} opacity-80`}>
            <h4 className="text-sm font-medium text-[var(--text-muted)]">3. {t("stepMoResult")}</h4>
            <p className="text-xs text-[var(--text-muted)]">{t("completePreviousStep")}</p>
          </section>
        )
      )}

      <section className={CARD_CLASS} aria-label="Step 4">
        <h4 className="text-sm font-medium text-[var(--text)] mb-2">4. {t("stepAdopt")}</h4>
        {runResult?.success ? (
          <div className="space-y-3">
            {diff != null && (
              <button
                type="button"
                onClick={handleAdoptToWorkspace}
                disabled={adoptLoading}
                className="px-4 py-2 bg-green-700 hover:bg-green-600 text-sm font-medium rounded-lg disabled:opacity-50"
              >
                {adoptLoading ? t("running") : t("adoptToWorkspace")}
              </button>
            )}
            {diff == null && (
              <>
                <label className="text-xs text-[var(--text-muted)] block mb-1">Commit message</label>
                <input
                  type="text"
                  value={commitMessage}
                  onChange={(e) => setCommitMessage(e.target.value)}
                  placeholder={t("commitMessagePlaceholder")}
                  className="w-full max-w-md bg-[#3c3c3c] border border-gray-600 px-3 py-2 text-sm rounded-lg"
                />
                <div className="mt-2">
                  <button
                    type="button"
                    onClick={handleCommitPatch}
                    disabled={commitLoading}
                    className="px-4 py-2 bg-[#3c3c3c] hover:bg-gray-600 text-sm font-medium rounded-lg border border-gray-600 disabled:opacity-50"
                  >
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

      <section ref={historySectionRef} className={fullScreen ? "flex-1 min-h-0 flex flex-col overflow-hidden mt-4" : "mt-4"}>
        <div className="flex items-center justify-between mb-2 shrink-0">
          <span className="text-sm font-medium text-[var(--text-muted)]">{t("iterationHistory")}</span>
          <div className="flex gap-2 items-center">
            {(["all", "pass", "fail"] as const).map((f) => (
              <button
                key={f}
                type="button"
                className={`px-3 py-1 text-xs rounded-lg ${historyFilter === f ? "bg-primary/30 text-primary" : "bg-[#3c3c3c] text-[var(--text-muted)] hover:text-[var(--text)]"}`}
                onClick={() => setHistoryFilter(f)}
              >
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
                    <tr
                      key={r.id}
                      className={`border-b border-gray-700/50 hover:bg-[#3c3c3c]/30 cursor-pointer ${expandedHistoryId === r.id ? "bg-[#3c3c3c]/30" : ""}`}
                      onClick={() => setExpandedHistoryId((prev) => (prev === r.id ? null : r.id))}
                    >
                      <td className="px-3 py-2 font-mono text-[var(--text)]">{r.id}</td>
                      <td className="px-3 py-2 text-[var(--text)] truncate max-w-[200px]" title={r.target}>
                        {r.target || "\u2014"}
                      </td>
                      <td className="px-3 py-2">
                        <span className={r.success ? "text-green-400" : "text-red-400"}>{r.success ? "Yes" : "No"}</span>
                      </td>
                      <td className="px-3 py-2 text-[var(--text-muted)]">{r.created_at.slice(0, 19)}</td>
                      <td className="px-3 py-2">{expandedHistoryId === r.id ? "\u25BC" : "\u25B6"}</td>
                    </tr>
                    {expandedHistoryId === r.id && (
                      <tr className="border-b border-gray-700/50 bg-[#252526]">
                        <td colSpan={5} className="px-3 py-2 text-[var(--text-muted)] text-xs whitespace-pre-wrap">
                          {r.message || "\u2014"}
                        </td>
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
