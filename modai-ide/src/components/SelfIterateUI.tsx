import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";

interface IterationRecord {
  id: number;
  target: string;
  diff: string | null;
  success: boolean;
  message: string;
  created_at: string;
}

interface SelfIterateUIProps {
  targetPrefill?: string | null;
  onClearPrefill?: () => void;
}

export function SelfIterateUI({ targetPrefill, onClearPrefill }: SelfIterateUIProps) {
  const [target, setTarget] = useState("");
  const [diff, setDiff] = useState<string | null>(null);
  useEffect(() => {
    if (targetPrefill) {
      setTarget(targetPrefill);
      onClearPrefill?.();
    }
  }, [targetPrefill, onClearPrefill]);
  const [patchLoading, setPatchLoading] = useState(false);
  const [runResult, setRunResult] = useState<{ success: boolean; build_ok: boolean; test_ok: boolean; message: string } | null>(null);
  const [runLoading, setRunLoading] = useState(false);
  const [history, setHistory] = useState<IterationRecord[]>([]);

  const loadHistory = useCallback(async () => {
    try {
      const list = (await invoke("list_iteration_history", { limit: 20 })) as IterationRecord[];
      setHistory(list);
    } catch {}
  }, []);

  useEffect(() => {
    loadHistory();
  }, [loadHistory]);

  const handleGeneratePatch = useCallback(async () => {
    if (!target.trim()) return;
    setPatchLoading(true);
    setDiff(null);
    setRunResult(null);
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
    try {
      const result = (await invoke("self_iterate", { diff: diff || undefined })) as {
        success: boolean;
        build_ok: boolean;
        test_ok: boolean;
        message: string;
      };
      setRunResult(result);
    } catch (e) {
      setRunResult({ success: false, build_ok: false, test_ok: false, message: String(e) });
    } finally {
      setRunLoading(false);
    }
  }, [diff]);

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

  return (
    <div className="mt-3 pt-3 border-t border-border">
      <div className="text-sm font-medium text-[var(--text-muted)] mb-2">{t("selfIterate")}</div>
      <div className="text-xs text-[var(--text-muted)] mb-1">{t("selfIterateTarget")}</div>
      <textarea
        value={target}
        onChange={(e) => setTarget(e.target.value)}
        placeholder="e.g. Add sparse Jacobian support"
        className="w-full h-14 bg-[#3c3c3c] border border-gray-600 px-2 py-1 text-sm resize-none rounded mb-2"
        rows={2}
      />
      <div className="flex gap-2 mb-2 flex-wrap">
        <button type="button" onClick={handleGeneratePatch} disabled={patchLoading} className="px-2 py-1 bg-primary hover:bg-blue-600 text-sm disabled:opacity-50 rounded">
          {patchLoading ? "..." : t("generatePatch")}
        </button>
        {diff != null && (
          <>
            <button type="button" onClick={handleRunInSandbox} disabled={runLoading} className="px-2 py-1 bg-gray-600 hover:bg-gray-500 text-sm rounded disabled:opacity-50">
              {runLoading ? "..." : t("runInSandbox")}
            </button>
            {runResult != null && (
              <button type="button" onClick={handleSaveToHistory} className="px-2 py-1 bg-gray-600 hover:bg-gray-500 text-sm rounded">
                {t("saveToHistory")}
              </button>
            )}
          </>
        )}
      </div>
      {diff != null && (
        <pre className="text-xs bg-[#1e1e1e] p-2 rounded border border-gray-700 overflow-auto max-h-32 whitespace-pre-wrap mb-2">
          {diff}
        </pre>
      )}
      {runResult != null && (
        <div className={`text-xs mb-2 p-2 rounded ${runResult.success ? "bg-green-900/30 text-green-300" : "bg-red-900/30 text-red-300"}`}>
          {runResult.message}
        </div>
      )}
      <div className="text-xs text-[var(--text-muted)] mb-1">{t("iterationHistory")}</div>
      <ul className="text-xs space-y-1 max-h-24 overflow-auto">
        {history.map((r) => (
          <li key={r.id} className="flex items-center gap-2">
            <span className={r.success ? "text-green-500" : "text-red-400"}>#{r.id}</span>
            <span className="truncate flex-1" title={r.target}>{r.target.slice(0, 40)}{r.target.length > 40 ? "..." : ""}</span>
            <span className="text-[var(--text-muted)]">{r.created_at.slice(0, 16)}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
