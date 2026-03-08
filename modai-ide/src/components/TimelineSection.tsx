import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";

export interface GitLogEntry {
  hash: string;
  subject: string;
  author: string;
  date: string;
}

interface TimelineSectionProps {
  projectDir: string | null;
  openFilePath: string | null;
  onOpenDiffAtRevision: (revision: string) => void;
}

export function TimelineSection({
  projectDir,
  openFilePath,
  onOpenDiffAtRevision,
}: TimelineSectionProps) {
  const [expanded, setExpanded] = useState(true);
  const [entries, setEntries] = useState<GitLogEntry[]>([]);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    if (!projectDir || !openFilePath) {
      setEntries([]);
      return;
    }
    setLoading(true);
    try {
      const pathForGit = openFilePath.replace(/\\/g, "/");
      const list = (await invoke("git_log", {
        projectDir,
        relativePath: pathForGit,
        limit: 20,
      })) as GitLogEntry[];
      setEntries(list);
    } catch {
      setEntries([]);
    } finally {
      setLoading(false);
    }
  }, [projectDir, openFilePath]);

  useEffect(() => {
    load();
  }, [load]);

  return (
    <div className="shrink-0 border-t border-border">
      <button
        type="button"
        className="tree-row w-full text-left font-medium text-[var(--text-muted)] hover:bg-white/5 rounded-none"
        style={{ paddingLeft: 8 }}
        onClick={() => setExpanded((e) => !e)}
        aria-expanded={expanded}
      >
        <span className="tree-arrow">{expanded ? "\u02C5" : "\u203A"}</span>
        <span className="tree-label">{t("timeline")}</span>
      </button>
      {expanded && (
        <div className="pb-2 px-2">
          {!projectDir || !openFilePath ? (
            <div className="text-xs text-[var(--text-muted)] px-1">{t("openProjectFirst")}</div>
          ) : loading ? (
            <div className="text-xs text-[var(--text-muted)] px-1">{t("running")}</div>
          ) : entries.length === 0 ? (
            <div className="text-xs text-[var(--text-muted)] px-1">{t("noCommitsYet")}</div>
          ) : (
            <ul className="text-xs space-y-0.5">
              {entries.map((entry) => (
                <li key={entry.hash}>
                  <button
                    type="button"
                    className="w-full text-left truncate px-1 py-0.5 rounded hover:bg-white/10 text-[var(--text)]"
                    onClick={() => onOpenDiffAtRevision(entry.hash)}
                    title={`${entry.subject} - ${entry.author} ${entry.date}`}
                  >
                    <span className="block truncate">{entry.subject || entry.hash.slice(0, 7)}</span>
                    <span className="text-[10px] text-[var(--text-muted)]">
                      {entry.hash.slice(0, 7)} \u00B7 {entry.author} \u00B7 {entry.date.slice(0, 10)}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}
