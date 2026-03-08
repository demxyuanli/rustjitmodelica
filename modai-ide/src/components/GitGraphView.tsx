import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";

export interface GitLogGraphEntry {
  hash: string;
  parents: string[];
  subject: string;
  author: string;
  date: string;
}

const ROW_HEIGHT = 28;
const NODE_R = 6;
const COLUMN_WIDTH = 24;

interface GitGraphViewProps {
  projectDir: string | null;
  onSelectCommit?: (hash: string) => void;
}

export function GitGraphView({ projectDir, onSelectCommit }: GitGraphViewProps) {
  const [entries, setEntries] = useState<GitLogGraphEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!projectDir) {
      setEntries([]);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const list = (await invoke("git_log_graph", { projectDir, limit: 80 })) as GitLogGraphEntry[];
      setEntries(list);
    } catch (e) {
      setError(String(e));
      setEntries([]);
    } finally {
      setLoading(false);
    }
  }, [projectDir]);

  useEffect(() => {
    load();
  }, [load]);

  if (!projectDir) {
    return (
      <div className="flex items-center justify-center h-full text-sm text-[var(--text-muted)]">
        {t("openProjectFirst")}
      </div>
    );
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full text-sm text-[var(--text-muted)]">
        {t("running")}
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-full text-sm text-red-400">
        {error}
      </div>
    );
  }

  if (entries.length === 0) {
    return (
      <div className="flex flex-col h-full min-h-0 bg-surface-alt">
        <div className="shrink-0 flex items-center justify-between gap-2 px-2 py-1 border-b border-border">
          <span className="text-xs font-medium text-[var(--text-muted)]">{t("graph")}</span>
          <button type="button" className="text-xs text-[var(--text-muted)] hover:text-[var(--text)]" onClick={load} title={t("refresh")}>
            {t("refresh")}
          </button>
        </div>
        <div className="flex-1 flex items-center justify-center text-sm text-[var(--text-muted)]">
          {t("noCommitsYet")}
        </div>
      </div>
    );
  }

  const hashToRow = new Map<string, number>();
  entries.forEach((e, i) => hashToRow.set(e.hash, i));

  const colMap = new Map<string, number>();
  let nextCol = 0;
  entries.forEach((e) => {
    const parents = e.parents.filter((p) => hashToRow.has(p));
    if (parents.length === 0) colMap.set(e.hash, nextCol++);
    else colMap.set(e.hash, colMap.get(parents[0]) ?? nextCol++);
  });
  const maxCol = nextCol || 1;
  const graphWidth = Math.max(COLUMN_WIDTH * maxCol, 48);
  const totalHeight = Math.max(entries.length * ROW_HEIGHT, ROW_HEIGHT);
  const listMinWidth = 120;

  return (
    <div className="flex flex-col h-full min-h-0 bg-surface-alt">
      <div className="shrink-0 flex items-center justify-between gap-2 px-2 py-1 border-b border-border">
        <span className="text-xs font-medium text-[var(--text-muted)]">{t("graph")}</span>
        <button
          type="button"
          className="text-xs text-[var(--text-muted)] hover:text-[var(--text)]"
          onClick={load}
          title={t("refresh")}
        >
          {t("refresh")}
        </button>
      </div>
      <div className="flex-1 min-h-0 overflow-auto scroll-vscode min-w-0">
        <div className="flex min-h-full w-max min-w-full">
          <svg width={graphWidth} height={totalHeight} className="shrink-0" aria-hidden>
            {entries.map((e, i) => {
              const cx = (colMap.get(e.hash) ?? 0) * COLUMN_WIDTH + COLUMN_WIDTH / 2;
              const cy = i * ROW_HEIGHT + ROW_HEIGHT / 2;
              const isMerge = e.parents.length > 1;
              return (
                <g key={e.hash}>
                  {e.parents.map((p, j) => {
                    const pr = hashToRow.get(p);
                    if (pr == null) return null;
                    const px = (colMap.get(p) ?? 0) * COLUMN_WIDTH + COLUMN_WIDTH / 2;
                    const py = pr * ROW_HEIGHT + ROW_HEIGHT / 2;
                    const midY = (cy + py) / 2;
                    const path =
                      j === 0
                        ? `M ${cx} ${cy} L ${cx} ${midY} L ${px} ${midY} L ${px} ${py}`
                        : `M ${cx} ${cy} L ${cx} ${midY} Q ${(cx + px) / 2} ${midY} ${px} ${midY} L ${px} ${py}`;
                    return (
                      <path
                        key={p + String(j)}
                        d={path}
                        fill="none"
                        stroke="var(--text-muted)"
                        strokeWidth="1.5"
                        opacity="0.8"
                      />
                    );
                  })}
                  <circle
                    cx={cx}
                    cy={cy}
                    r={NODE_R}
                    fill={isMerge ? "var(--primary)" : "var(--text-muted)"}
                    className="cursor-pointer hover:opacity-80"
                    onClick={() => onSelectCommit?.(e.hash)}
                  />
                </g>
              );
            })}
          </svg>
          <div
            className="flex flex-col border-l border-border pl-2 flex-1 min-w-0 overflow-hidden"
            style={{ minWidth: listMinWidth }}
          >
            {entries.map((e) => (
              <div
                key={e.hash}
                className="flex items-center gap-2 cursor-pointer hover:bg-white/5 rounded px-1 py-0.5 min-h-0"
                style={{ minHeight: ROW_HEIGHT }}
                onClick={() => onSelectCommit?.(e.hash)}
                role="button"
                tabIndex={0}
                onKeyDown={(ev) => ev.key === "Enter" && onSelectCommit?.(e.hash)}
              >
                <span className="text-xs text-[var(--text)] truncate flex-1 min-w-0" title={e.subject}>
                  {e.subject || e.hash.slice(0, 7)}
                </span>
                <span className="text-[10px] text-[var(--text-muted)] shrink-0">{e.author}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
