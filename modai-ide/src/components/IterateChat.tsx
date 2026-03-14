import { useCallback, useEffect, useMemo, useState, type Dispatch, type SetStateAction } from "react";
import {
  aiGenerateCompilerPatch,
  aiGenerateCompilerPatchWithContext,
  applyPatchToWorkspace,
  commitIterationPatch,
  getIteration,
  gitHeadCommit,
  indexRepoGetContext,
  listIterationHistory,
  runSelfIterate,
  saveIteration,
  type IterationRecord,
  type IterationRunResult,
} from "../api/tauri";
import { getCases, getSourceModules } from "../data/jit_regression_metadata";
import { t, tf } from "../i18n";
import { IterateActions } from "./IterateActions";
import { IterateDiffPreview } from "./IterateDiffPreview";
import { IterateHistory } from "./IterateHistory";

interface IterateChatProps {
  targetPrefill?: string | null;
  onClearPrefill?: () => void;
  repoRoot?: string | null;
  initialContextFiles?: string[];
  onDiffGenerated?: (diff: string) => void;
  onRunResult?: (result: unknown) => void;
  onViewIterationDiff?: (iterationId: number, unifiedDiff: string, title?: string) => void;
}

interface ChatEntry {
  id: number;
  role: "user" | "assistant" | "system";
  text: string;
  diff?: string | null;
  runResult?: IterationRunResult | null;
}

function createId() {
  return Date.now() + Math.floor(Math.random() * 1000);
}

function buildIterateMessage(target: string) {
  return `${tf("preparedCompilerPatch", { target })} ${t("patchReadyReviewRun")}`;
}

function getRoleLabel(role: ChatEntry["role"]): string {
  switch (role) {
    case "user":
      return t("roleUser");
    case "assistant":
      return t("roleAssistant");
    case "system":
      return t("roleSystem");
    default:
      return role;
  }
}

export function IterateChat({
  targetPrefill,
  onClearPrefill,
  repoRoot,
  initialContextFiles,
  onDiffGenerated,
  onRunResult,
  onViewIterationDiff,
}: IterateChatProps) {
  const [target, setTarget] = useState("");
  const [messages, setMessages] = useState<ChatEntry[]>([]);
  const [contextFiles, setContextFiles] = useState<string[]>([]);
  const [testCases, setTestCases] = useState<string[]>([]);
  const [history, setHistory] = useState<IterationRecord[]>([]);
  const [diff, setDiff] = useState<string | null>(null);
  const [runResult, setRunResult] = useState<IterationRunResult | null>(null);
  const [patchLoading, setPatchLoading] = useState(false);
  const [runLoading, setRunLoading] = useState(false);
  const [adoptLoading, setAdoptLoading] = useState(false);
  const [commitLoading, setCommitLoading] = useState(false);
  const [commitMessage, setCommitMessage] = useState("");
  const [banner, setBanner] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [advancedOpen, setAdvancedOpen] = useState(false);

  const sourceModules = useMemo(() => Object.keys(getSourceModules()).sort(), []);
  const cases = useMemo(() => getCases(), []);

  const loadHistory = useCallback(async () => {
    try {
      const records = await listIterationHistory(30);
      setHistory(records);
    } catch {
      setHistory([]);
    }
  }, []);

  useEffect(() => {
    loadHistory();
  }, [loadHistory]);

  useEffect(() => {
    if (targetPrefill) {
      setTarget(targetPrefill);
      onClearPrefill?.();
    }
  }, [targetPrefill, onClearPrefill]);

  useEffect(() => {
    if (initialContextFiles && initialContextFiles.length > 0) {
      setContextFiles((prev) => {
        const next = [...prev];
        for (const item of initialContextFiles) {
          if (!next.includes(item)) next.push(item);
        }
        return next;
      });
    }
  }, [initialContextFiles]);

  useEffect(() => {
    if (!banner || banner.type !== "success") return;
    const timer = window.setTimeout(() => setBanner(null), 3500);
    return () => window.clearTimeout(timer);
  }, [banner]);

  const pushMessage = useCallback((entry: ChatEntry) => {
    setMessages((prev) => [...prev, entry]);
  }, []);

  const toggleValue = useCallback((value: string, setter: Dispatch<SetStateAction<string[]>>) => {
    setter((prev) => (prev.includes(value) ? prev.filter((item) => item !== value) : [...prev, value]));
  }, []);

  const runSmartContext = useCallback(async (query: string) => {
    try {
      const chunks = (await indexRepoGetContext(query, 8)) as Array<{ filePath: string }>;
      const files = [...new Set(chunks.map((chunk) => chunk.filePath).filter(Boolean))];
      if (files.length > 0) {
        setContextFiles((prev) => {
          const merged = [...prev];
          for (const file of files) {
            if (!merged.includes(file)) merged.push(file);
          }
          return merged;
        });
      }
      return files;
    } catch {
      return [];
    }
  }, []);

  const handleGenerate = useCallback(async () => {
    const nextTarget = target.trim();
    if (!nextTarget) return;

    setPatchLoading(true);
    setRunResult(null);
    setDiff(null);
    setBanner(null);
    pushMessage({ id: createId(), role: "user", text: nextTarget });

    try {
      const smartFiles = await runSmartContext(nextTarget);
      const mergedContext = [...new Set([...contextFiles, ...smartFiles])];
      const result = mergedContext.length > 0 || testCases.length > 0
        ? await aiGenerateCompilerPatchWithContext(nextTarget, mergedContext, testCases)
        : await aiGenerateCompilerPatch(nextTarget);
      setDiff(result);
      onDiffGenerated?.(result);
      pushMessage({
        id: createId(),
        role: "assistant",
        text: buildIterateMessage(nextTarget),
        diff: result,
      });
      setBanner({ type: "success", text: t("patchReadyReviewRun") });
    } catch (error) {
      const text = String(error);
      pushMessage({ id: createId(), role: "system", text });
      setBanner({ type: "error", text });
    } finally {
      setPatchLoading(false);
    }
  }, [contextFiles, onDiffGenerated, pushMessage, runSmartContext, target, testCases]);

  const handleRun = useCallback(async (quick: boolean) => {
    if (!diff) return;
    setRunLoading(true);
    setBanner(null);
    try {
      const result = await runSelfIterate(diff, quick);
      setRunResult(result);
      onRunResult?.(result);
      pushMessage({
        id: createId(),
        role: "system",
        text: result.message,
        runResult: result,
      });
      if (!quick || !result.success) {
        let gitCommit: string | null = null;
        if (repoRoot) {
          try {
            gitCommit = await gitHeadCommit(repoRoot);
          } catch {
            gitCommit = null;
          }
        }
        await saveIteration(target.trim(), diff, result.success, result.message, gitCommit);
        await loadHistory();
      }
    } catch (error) {
      const text = String(error);
      setBanner({ type: "error", text });
      pushMessage({ id: createId(), role: "system", text });
    } finally {
      setRunLoading(false);
    }
  }, [diff, loadHistory, onRunResult, pushMessage, repoRoot, target]);

  const handleAdopt = useCallback(async () => {
    if (!diff) return;
    setAdoptLoading(true);
    setBanner(null);
    try {
      await applyPatchToWorkspace(diff);
      setDiff(null);
      setBanner({ type: "success", text: t("adoptedSuccess") });
      pushMessage({ id: createId(), role: "system", text: t("adoptedSuccess") });
    } catch (error) {
      const text = String(error);
      setBanner({ type: "error", text });
    } finally {
      setAdoptLoading(false);
    }
  }, [diff, pushMessage]);

  const handleCommit = useCallback(async () => {
    setCommitLoading(true);
    setBanner(null);
    try {
      await commitIterationPatch(commitMessage.trim() || t("commitMessagePlaceholder"));
      setBanner({ type: "success", text: t("committedSuccess") });
      pushMessage({ id: createId(), role: "system", text: t("committedSuccess") });
      await loadHistory();
    } catch (error) {
      const text = String(error);
      setBanner({ type: "error", text });
    } finally {
      setCommitLoading(false);
    }
  }, [commitMessage, loadHistory, pushMessage]);

  const handleReuseHistory = useCallback(async (record: IterationRecord) => {
    let nextRecord = record;
    if (!record.diff) {
      const fresh = await getIteration(record.id);
      if (!fresh?.diff) return;
      nextRecord = fresh;
    }
    setTarget(nextRecord.target || "");
    setDiff(nextRecord.diff ?? null);
    setRunResult(null);
    pushMessage({
      id: createId(),
      role: "system",
      text: tf("loadedPatchFromHistory", { id: nextRecord.id }),
      diff: nextRecord.diff,
    });
  }, [pushMessage]);

  const canRunFull = !!runResult?.quick_run && !!runResult?.success;
  const canAdopt = !!runResult?.success && !!diff;
  const canCommit = !!runResult?.success && !diff;

  return (
    <div className="flex flex-col h-full min-h-0 overflow-hidden bg-surface-alt">
      {banner && (
        <div
          className={`mx-4 mt-4 rounded-lg border px-3 py-2 text-xs ${
            banner.type === "error"
              ? "theme-banner-danger"
              : "theme-banner-success"
          }`}
        >
          {banner.text}
        </div>
      )}

      <div className="flex-1 min-h-0 overflow-auto px-4 py-4 space-y-4">
        {messages.length === 0 && (
          <div className="rounded-lg border border-dashed border-border px-4 py-6 text-sm text-[var(--text-muted)]">
            {t("describeCompilerCapability")}
          </div>
        )}

        {messages.map((message) => (
          <div key={message.id} className="space-y-2">
            <div
              className={`rounded-xl px-4 py-3 text-sm ${
                message.role === "user"
                  ? "bg-primary/20 border border-primary/30 ml-8"
                  : message.role === "assistant"
                    ? "bg-[var(--surface-elevated)] border border-border mr-8"
                    : "bg-[var(--panel-muted-bg)] border border-border"
              }`}
            >
              <div className="text-[10px] uppercase tracking-wide text-[var(--text-muted)] mb-1">
                {getRoleLabel(message.role)}
              </div>
              <div className="whitespace-pre-wrap break-words">{message.text}</div>
            </div>
            {message.diff && <IterateDiffPreview diff={message.diff} />}
            {message.runResult?.mo_run && message.runResult.mo_run.details.length > 0 && (
              <div className="rounded-lg border border-border bg-[var(--panel-muted-bg)] overflow-hidden">
                <div className="px-3 py-2 border-b border-border text-xs text-[var(--text-muted)]">
                  {tf("moCasesFailed", { passed: message.runResult.mo_run.passed, failed: message.runResult.mo_run.failed })}
                </div>
                <div className="max-h-48 overflow-auto">
                  <table className="w-full text-xs">
                    <thead>
                      <tr className="text-left text-[var(--text-muted)] bg-[var(--surface-elevated)] border-b border-border">
                        <th className="px-3 py-2 font-medium">{t("viewByCase")}</th>
                        <th className="px-3 py-2 font-medium">{t("expected")}</th>
                        <th className="px-3 py-2 font-medium">{t("actual")}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {message.runResult.mo_run.details.map((detail) => (
                        <tr key={detail.name} className="border-b border-border/60 last:border-b-0">
                          <td className="px-3 py-2">{detail.name}</td>
                          <td className="px-3 py-2">{detail.expected}</td>
                          <td className={`px-3 py-2 ${detail.actual === detail.expected ? "text-[var(--success-text)]" : "text-[var(--danger-text)]"}`}>
                            {detail.actual}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            )}
          </div>
        ))}

        {(diff || runResult) && (
          <div className="rounded-lg border border-border bg-[var(--panel-muted-bg)] p-4 space-y-3">
            <div className="text-xs font-medium text-[var(--text)]">{t("nextActions")}</div>
            {diff && <IterateDiffPreview diff={diff} title={t("currentPatch")} defaultExpanded />}
            <IterateActions
              runLoading={runLoading}
              adoptLoading={adoptLoading}
              commitLoading={commitLoading}
              canRunFull={canRunFull}
              canAdopt={canAdopt}
              canCommit={canCommit}
              commitMessage={commitMessage}
              onCommitMessageChange={setCommitMessage}
              onRunQuick={() => handleRun(true)}
              onRunFull={() => handleRun(false)}
              onAdopt={handleAdopt}
              onCommit={handleCommit}
            />
          </div>
        )}

        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <div className="text-xs font-medium text-[var(--text)]">{t("advancedContext")}</div>
            <button
              type="button"
              onClick={() => setAdvancedOpen((value) => !value)}
              className="text-xs text-[var(--text-muted)] hover:text-[var(--text)]"
            >
              {advancedOpen ? t("hide") : t("show")}
            </button>
          </div>
          {advancedOpen && (
            <div className="grid grid-cols-2 gap-3">
              <div className="rounded-lg border border-border bg-[var(--surface)] max-h-48 overflow-auto p-2">
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("sourceContext")}</div>
                {sourceModules.map((path) => (
                  <label key={path} className="flex items-center gap-2 py-1 text-[11px] cursor-pointer">
                    <input
                      type="checkbox"
                      checked={contextFiles.includes(path)}
                      onChange={() => toggleValue(path, setContextFiles)}
                    />
                    <span className="font-mono truncate">{path}</span>
                  </label>
                ))}
              </div>
              <div className="rounded-lg border border-border bg-[var(--surface)] max-h-48 overflow-auto p-2">
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("regressionContext")}</div>
                {cases.map((testCase) => (
                  <label key={testCase.name} className="flex items-center gap-2 py-1 text-[11px] cursor-pointer">
                    <input
                      type="checkbox"
                      checked={testCases.includes(testCase.name)}
                      onChange={() => toggleValue(testCase.name, setTestCases)}
                    />
                    <span className="truncate">{testCase.name}</span>
                  </label>
                ))}
              </div>
            </div>
          )}
        </div>

        <IterateHistory
          history={history}
          onReuseDiff={handleReuseHistory}
          onViewDiff={onViewIterationDiff
            ? (record) => onViewIterationDiff(record.id, record.diff ?? "", `#${record.id} ${(record.target || t("currentPatch")).slice(0, 40)}`)
            : undefined}
        />
      </div>

      <div className="border-t border-border p-4 shrink-0">
        <div className="rounded-xl border border-border bg-[var(--panel-muted-bg)] p-3">
          <textarea
            value={target}
            onChange={(event) => setTarget(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                if (!patchLoading && target.trim()) handleGenerate();
              }
            }}
            placeholder={t("step1DescribeGoal")}
            className="w-full bg-transparent resize-none outline-none text-sm min-h-[84px]"
          />
          <div className="mt-3 flex items-center justify-between gap-3 flex-wrap">
            <div className="text-[11px] text-[var(--text-muted)]">
              {t("smartContextAuto")}
            </div>
            <button
              type="button"
              onClick={handleGenerate}
              disabled={patchLoading || !target.trim()}
              className="px-4 py-2 rounded-lg bg-primary hover:bg-blue-600 text-white text-sm font-medium disabled:opacity-50"
            >
              {patchLoading ? t("generating") : t("generatePatch")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
