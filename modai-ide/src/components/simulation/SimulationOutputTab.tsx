import { useEffect, useMemo, useState } from "react";
import {
  formatMonitorReplayLine,
  getMonitorEvents,
  listMonitorEventSessions,
  type MonitorEventSessionEntry,
} from "../../api/tauri";
import { t, tf } from "../../i18n";

const DEFAULT_REPLAY_LIMIT = 500;
const SESSION_LIST_LIMIT = 80;

export interface SimulationOutputTabProps {
  logSearch: string;
  onLogSearchChange: (value: string) => void;
  logLines: string[];
  onClearLog?: () => void;
  onAppendLogLines?: (lines: string[]) => void;
  onOutputContextMenu: (x: number, y: number) => void;
}

export function SimulationOutputTab({
  logSearch,
  onLogSearchChange,
  logLines,
  onClearLog,
  onAppendLogLines,
  onOutputContextMenu,
}: SimulationOutputTabProps) {
  const [showControl, setShowControl] = useState(true);
  const [showProgress, setShowProgress] = useState(true);
  const [showError, setShowError] = useState(true);
  const [showOther, setShowOther] = useState(true);
  const [progressExpanded, setProgressExpanded] = useState(false);
  const [replayBusy, setReplayBusy] = useState(false);
  const [replaySessionId, setReplaySessionId] = useState("");
  const [replaySessions, setReplaySessions] = useState<MonitorEventSessionEntry[]>([]);
  const [sessionsBusy, setSessionsBusy] = useState(false);
  const [replayLimitInput, setReplayLimitInput] = useState(String(DEFAULT_REPLAY_LIMIT));

  useEffect(() => {
    if (!onAppendLogLines) return;
    let cancelled = false;
    setSessionsBusy(true);
    void listMonitorEventSessions(SESSION_LIST_LIMIT)
      .then((rows) => {
        if (!cancelled) setReplaySessions(rows);
      })
      .catch(() => {
        if (!cancelled) setReplaySessions([]);
      })
      .finally(() => {
        if (!cancelled) setSessionsBusy(false);
      });
    return () => {
      cancelled = true;
    };
  }, [onAppendLogLines]);

  function refreshSessionList() {
    if (!onAppendLogLines) return;
    setSessionsBusy(true);
    void listMonitorEventSessions(SESSION_LIST_LIMIT)
      .then((rows) => setReplaySessions(rows))
      .catch(() => setReplaySessions([]))
      .finally(() => setSessionsBusy(false));
  }

  const replayLimit = useMemo(() => {
    const n = Number.parseInt(replayLimitInput.trim(), 10);
    if (!Number.isFinite(n)) return DEFAULT_REPLAY_LIMIT;
    return Math.min(1000, Math.max(1, n));
  }, [replayLimitInput]);

  const filtered = logLines.filter(
    (line) =>
      !logSearch.trim() ||
      line.toLowerCase().includes(logSearch.trim().toLowerCase())
  );
  const controlLines = useMemo(() => filtered.filter((line) => line.startsWith("[control]")), [filtered]);
  const progressLines = useMemo(() => filtered.filter((line) => line.startsWith("[progress]")), [filtered]);
  const errorLines = useMemo(() => filtered.filter((line) => line.startsWith("[error]")), [filtered]);
  const otherLines = useMemo(() => filtered.filter(
    (line) =>
      !line.startsWith("[control]") &&
      !line.startsWith("[progress]") &&
      !line.startsWith("[error]")
  ), [filtered]);
  const shownProgressLines = progressExpanded ? progressLines : progressLines.slice(-100);

  async function replayMonitorEvents(sessionId: string | null) {
    if (!onAppendLogLines) return;
    setReplayBusy(true);
    try {
      const rows = await getMonitorEvents(sessionId, replayLimit);
      const label =
        sessionId == null || sessionId === ""
          ? "latest file"
          : `session=${sessionId}`;
      const header = `[monitor-replay] manual replay ${label} (${rows.length} events):`;
      const lines =
        rows.length === 0
          ? [header, "[monitor-replay] no persisted events"]
          : [header, ...rows.map(formatMonitorReplayLine)];
      onAppendLogLines(lines);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      onAppendLogLines([`[monitor-replay] failed: ${msg}`]);
    } finally {
      setReplayBusy(false);
    }
  }

  function renderGroup(title: string, lines: string[], tone: string) {
    if (lines.length === 0) return null;
    return (
      <div className="mb-3">
        <div className={`mb-1 text-[10px] uppercase tracking-wide ${tone}`}>{title} ({lines.length})</div>
        {lines.map((line, i) => (
          <div key={`${title}-${i}`} className="py-0.5 leading-tight text-[var(--text-muted)]">
            {line}
          </div>
        ))}
      </div>
    );
  }

  return (
    <div className="flex flex-1 min-h-0 flex-col">
      <div className="flex shrink-0 items-center gap-2 border-b border-border px-2 py-1">
        <input
          type="text"
          placeholder={t("tableSearch")}
          value={logSearch}
          onChange={(e) => onLogSearchChange(e.target.value)}
          className="max-w-xs flex-1 rounded border border-border bg-surface px-2 py-0.5 text-xs"
        />
        <button
          type="button"
          className="ml-auto rounded border border-border px-2 py-0.5 text-xs theme-button-secondary"
          onClick={onClearLog}
          disabled={!onClearLog || logLines.length === 0}
        >
          {t("clearLog")}
        </button>
        <button
          type="button"
          className="rounded border border-border px-2 py-0.5 text-xs theme-button-secondary"
          onClick={() => {
            const lines: string[] = [];
            if (showControl) lines.push(...controlLines);
            if (showProgress) lines.push(...shownProgressLines);
            if (showError) lines.push(...errorLines);
            if (showOther) lines.push(...otherLines);
            void navigator.clipboard.writeText(lines.join("\n"));
          }}
          disabled={filtered.length === 0}
        >
          copy
        </button>
      </div>
      <div className="flex shrink-0 items-center gap-2 border-b border-border px-2 py-1 text-[10px]">
        <label className="flex items-center gap-1"><input type="checkbox" checked={showControl} onChange={(e) => setShowControl(e.target.checked)} />control ({controlLines.length})</label>
        <label className="flex items-center gap-1"><input type="checkbox" checked={showProgress} onChange={(e) => setShowProgress(e.target.checked)} />progress ({progressLines.length})</label>
        <label className="flex items-center gap-1"><input type="checkbox" checked={showError} onChange={(e) => setShowError(e.target.checked)} />error ({errorLines.length})</label>
        <label className="flex items-center gap-1"><input type="checkbox" checked={showOther} onChange={(e) => setShowOther(e.target.checked)} />other ({otherLines.length})</label>
        {progressLines.length > 100 && (
          <button
            type="button"
            className="ml-auto rounded border border-border px-2 py-0.5 text-[10px] theme-button-secondary"
            onClick={() => setProgressExpanded((v) => !v)}
          >
            {progressExpanded ? "collapse progress" : "expand progress"}
          </button>
        )}
      </div>
      {onAppendLogLines && (
        <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-border px-2 py-1 text-[10px]">
          <label className="flex items-center gap-1">
            <span className="text-[var(--text-muted)]">{t("outputReplayMaxEvents")}</span>
            <input
              type="number"
              min={1}
              max={1000}
              value={replayLimitInput}
              onChange={(e) => setReplayLimitInput(e.target.value)}
              className="w-16 rounded border border-border bg-surface px-1 py-0.5 text-[10px]"
            />
          </label>
          <button
            type="button"
            className="rounded border border-border px-2 py-0.5 theme-button-secondary"
            disabled={replayBusy}
            onClick={() => void replayMonitorEvents(null)}
          >
            {t("outputReplayLatest")}
          </button>
          <input
            type="text"
            placeholder={t("outputSessionIdPlaceholder")}
            value={replaySessionId}
            onChange={(e) => setReplaySessionId(e.target.value)}
            list="modai-monitor-replay-sessions"
            className="min-w-[8rem] max-w-[14rem] flex-1 rounded border border-border bg-surface px-2 py-0.5 text-[10px]"
          />
          <datalist id="modai-monitor-replay-sessions">
            {replaySessions.map((s) => (
              <option
                key={s.sessionId}
                value={s.sessionId}
                label={`${s.eventCount} ${s.modifiedMs != null ? new Date(s.modifiedMs).toISOString() : ""}`}
              />
            ))}
          </datalist>
          <button
            type="button"
            className="rounded border border-border px-2 py-0.5 theme-button-secondary"
            disabled={replayBusy || !replaySessionId.trim()}
            onClick={() => void replayMonitorEvents(replaySessionId.trim())}
          >
            {t("outputReplaySession")}
          </button>
          <button
            type="button"
            className="rounded border border-border px-2 py-0.5 theme-button-secondary"
            disabled={sessionsBusy}
            onClick={refreshSessionList}
            title={t("outputRefreshSessionList")}
          >
            {t("outputRefreshSessionList")}
          </button>
        </div>
      )}
      {onAppendLogLines && replaySessions.length > 0 && (
        <div className="flex max-h-24 shrink-0 flex-col gap-0.5 overflow-y-auto border-b border-border px-2 py-1 text-[10px] scroll-vscode">
          <div>
            <div className="text-[var(--text-muted)]">{t("outputReplaySessionsTitle")}</div>
            <div className="text-[var(--text-muted)] opacity-80">{t("outputReplaySessionsSubtitle")}</div>
          </div>
          {replaySessions.map((s) => (
            <button
              key={s.sessionId}
              type="button"
              className="truncate rounded px-1 py-0.5 text-left font-mono hover:bg-[var(--surface-hover)] disabled:opacity-50"
              disabled={replayBusy}
              title={
                `${t("outputReplaySessionRowHint")} · ${t("outputReplaySessionCopyHint")}${
                  s.modifiedMs != null ? ` — ${new Date(s.modifiedMs).toISOString()}` : ""
                }`
              }
              onClick={() => {
                setReplaySessionId(s.sessionId);
                void replayMonitorEvents(s.sessionId);
              }}
              onContextMenu={(e) => {
                e.preventDefault();
                e.stopPropagation();
                void navigator.clipboard.writeText(s.sessionId).then(() => {
                  onAppendLogLines?.([
                    `[monitor-replay] copied session id: ${s.sessionId}`,
                  ]);
                });
              }}
            >
              <span className="text-[var(--text)]">{s.sessionId}</span>
              <span className="text-[var(--text-muted)]">
                {" "}
                &middot; {tf("outputReplayEventsCount", { count: s.eventCount })}
                {s.modifiedMs != null && (
                  <>
                    {" "}
                    &middot; {new Date(s.modifiedMs).toISOString().replace("T", " ").slice(0, 19)}
                  </>
                )}
              </span>
            </button>
          ))}
        </div>
      )}
      <div
        className="flex-1 overflow-auto p-2 font-mono text-xs scroll-vscode"
        onContextMenu={(event) => {
          event.preventDefault();
          onOutputContextMenu(event.clientX, event.clientY);
        }}
      >
        {logLines.length === 0 ? (
          <div className="text-[var(--text-muted)]">{t("tabOutput")}</div>
        ) : (
          <>
            {showControl && renderGroup("Control", controlLines, "text-sky-300")}
            {showProgress && renderGroup("Progress", shownProgressLines, "text-[var(--text-muted)]")}
            {showError && renderGroup("Error", errorLines, "text-red-300")}
            {showOther && renderGroup("Other", otherLines, "text-[var(--text-muted)]")}
          </>
        )}
      </div>
    </div>
  );
}
