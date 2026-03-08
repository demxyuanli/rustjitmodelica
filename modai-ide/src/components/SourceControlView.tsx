import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";

export interface GitStatus {
  branch: string;
  staged: string[];
  modified: string[];
  deleted: string[];
  untracked: string[];
  renamed: { from: string; to: string }[];
}

export interface GitLogEntry {
  hash: string;
  subject: string;
  author: string;
  date: string;
}

export interface GitCommitFile {
  status: string;
  path: string;
}

interface SourceControlViewProps {
  projectDir: string | null;
  onOpenDiff: (relativePath: string, isStaged: boolean) => void;
  onOpenInEditor?: (relativePath: string) => void;
  onRefreshStatus?: () => void;
}

interface ChangeNode {
  name: string;
  fullPath?: string;
  status?: string;
  isStaged?: boolean;
  children: Map<string, ChangeNode>;
}

function buildTree(paths: { path: string; status: string; isStaged: boolean }[]): ChangeNode {
  const root: ChangeNode = { name: "", children: new Map() };
  for (const { path, status, isStaged } of paths) {
    const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
    let cur = root;
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      if (!cur.children.has(part)) {
        cur.children.set(part, { name: part, children: new Map() });
      }
      cur = cur.children.get(part)!;
      if (i === parts.length - 1) {
        cur.fullPath = path;
        cur.status = status;
        cur.isStaged = isStaged;
      }
    }
  }
  return root;
}

function sortNodes(nodes: [string, ChangeNode][]): [string, ChangeNode][] {
  return [...nodes].sort(([a, aNode], [b, bNode]) => {
    const aIsFile = aNode.fullPath != null;
    const bIsFile = bNode.fullPath != null;
    if (aIsFile !== bIsFile) return aIsFile ? 1 : -1;
    return a.localeCompare(b, undefined, { sensitivity: "base" });
  });
}

function fileIcon(seg: string): string {
  const ext = seg.includes(".") ? seg.split(".").pop()?.toLowerCase() : "";
  if (ext === "tsx" || ext === "ts") return "T";
  if (ext === "json") return "J";
  if (ext === "mo") return "M";
  if (ext === "h" || ext === "c" || ext === "cpp" || ext === "rs") return "C";
  if (ext === "md" || ext === "txt") return "T";
  return "F";
}

function statusRowClass(status: string, isStaged: boolean): string {
  const s = status === "A" || (status === "M" && isStaged) ? (isStaged ? "A" : "M") : status;
  return `scm-tree-row scm-status-${s}${isStaged ? " scm-staged" : ""}`;
}

const TREE_INDENT = 14;
const TREE_BASE = 8;

export function SourceControlView({
  projectDir,
  onOpenDiff,
  onOpenInEditor,
  onRefreshStatus,
}: SourceControlViewProps) {
  const [isRepo, setIsRepo] = useState(false);
  const [status, setStatus] = useState<GitStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [commitMessage, setCommitMessage] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [initLoading, setInitLoading] = useState(false);
  const [expandedDirs, setExpandedDirs] = useState<Set<string>>(new Set());
  const [stagedOpen, setStagedOpen] = useState(true);
  const [changesOpen, setChangesOpen] = useState(true);

  const refresh = useCallback(async () => {
    if (!projectDir) {
      setIsRepo(false);
      setStatus(null);
      setError(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const repo = (await invoke("git_is_repo", { projectDir })) as boolean;
      setIsRepo(repo);
      if (!repo) {
        setStatus(null);
        onRefreshStatus?.();
        return;
      }
      const s = (await invoke("git_status", { projectDir })) as GitStatus;
      setStatus(s);
      onRefreshStatus?.();
    } catch (e) {
      setError(String(e));
      setStatus(null);
    } finally {
      setLoading(false);
    }
  }, [projectDir, onRefreshStatus]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleStage = useCallback(
    async (paths: string[]) => {
      if (!projectDir || paths.length === 0) return;
      try {
        await invoke("git_stage", { projectDir, paths });
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [projectDir, refresh]
  );

  const handleUnstage = useCallback(
    async (paths: string[]) => {
      if (!projectDir || paths.length === 0) return;
      try {
        await invoke("git_unstage", { projectDir, paths });
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [projectDir, refresh]
  );

  const handleCommit = useCallback(async () => {
    if (!projectDir || !commitMessage.trim()) return;
    try {
      await invoke("git_commit", { projectDir, message: commitMessage.trim() });
      setCommitMessage("");
      await refresh();
      onRefreshStatus?.();
    } catch (e) {
      setError(String(e));
    }
  }, [projectDir, commitMessage, refresh, onRefreshStatus]);

  const handleInitGit = useCallback(async () => {
    if (!projectDir || initLoading) return;
    setInitLoading(true);
    setError(null);
    try {
      await invoke("git_init", { projectDir });
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setInitLoading(false);
    }
  }, [projectDir, initLoading, refresh]);

  useEffect(() => {
    if (!status) return;
    const paths = [
      ...status.staged,
      ...status.modified,
      ...status.deleted,
      ...status.untracked,
      ...status.renamed.map((r) => r.to),
    ];
    const prefixes = new Set<string>();
    paths.forEach((path) => {
      const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
      for (let i = 1; i < parts.length; i++) {
        prefixes.add(parts.slice(0, i).join("/"));
      }
    });
    setExpandedDirs(prefixes);
  }, [status]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
        if (status && commitMessage.trim() && status.staged.length > 0) handleCommit();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [status, commitMessage, handleCommit]);

  if (!projectDir) {
    return (
      <div className="p-3 text-sm text-[var(--text-muted)]">
        {t("openProjectFirst")}
      </div>
    );
  }

  if (loading && !status) {
    return (
      <div className="p-3 text-sm text-[var(--text-muted)]">{t("running")}</div>
    );
  }

  if (!isRepo || !status) {
    return (
      <div className="p-3 flex flex-col gap-2">
        <p className="text-sm text-[var(--text-muted)]">{error || t("notGitRepo")}</p>
        <button
          type="button"
          className="self-start px-3 py-1.5 text-sm bg-primary text-white rounded hover:opacity-90 disabled:opacity-50"
          onClick={handleInitGit}
          disabled={initLoading}
        >
          {initLoading ? t("running") : t("initGitRepo")}
        </button>
      </div>
    );
  }

  const stagedPaths: { path: string; status: string; isStaged: boolean }[] = [
    ...status.staged.map((path) => ({ path, status: "M", isStaged: true })),
    ...status.renamed.map((r) => ({ path: r.to, status: "R", isStaged: true })),
  ];
  const unstagedPaths: { path: string; status: string; isStaged: boolean }[] = [];
  status.modified.forEach((path) => {
    if (!status.staged.includes(path) && !status.renamed.some((r) => r.to === path)) {
      unstagedPaths.push({ path, status: "M", isStaged: false });
    }
  });
  status.deleted.forEach((path) => {
    if (!status.staged.includes(path)) {
      unstagedPaths.push({ path, status: "D", isStaged: false });
    }
  });
  status.untracked.forEach((path) => {
    unstagedPaths.push({ path, status: "U", isStaged: false });
  });

  const stagedTree = buildTree(stagedPaths);
  const changesTree = buildTree(unstagedPaths);
  const stagedCount = stagedPaths.length;
  const changesCount = unstagedPaths.length;
  const hasStaged = stagedCount > 0;

  function toggleExpanded(key: string) {
    setExpandedDirs((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  function FileTree({
    node,
    depth,
    pathPrefix,
    onStage,
    onUnstage,
  }: {
    node: ChangeNode;
    depth: number;
    pathPrefix: string;
    onStage: (path: string) => void;
    onUnstage: (path: string) => void;
  }) {
    const entries = sortNodes(Array.from(node.children.entries()));
    if (entries.length === 0) return null;
    const paddingLeft = TREE_BASE + depth * TREE_INDENT;
    return (
      <>
        {entries.map(([seg, child]) => {
          const key = pathPrefix ? `${pathPrefix}/${seg}` : seg;
          const isFile = child.fullPath != null;
          const hasChildren = child.children.size > 0;
          const isExpanded = expandedDirs.has(key);
          const status = child.status ?? "M";
          const staged = child.isStaged ?? false;
          const statusBadgeClass = staged ? "scm-status-badge staged" : `scm-status-badge ${status === "M" ? "modified" : status === "U" ? "untracked" : status === "D" ? "deleted" : status === "R" ? "renamed" : "added"}`;
          return (
            <div key={key} className="flex flex-col">
              <div
                className={`tree-row group rounded ${isFile ? statusRowClass(status, staged) : ""}`}
                style={{ paddingLeft }}
              >
                {hasChildren ? (
                  <button
                    type="button"
                    className="tree-arrow text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10 rounded"
                    onClick={() => toggleExpanded(key)}
                    aria-expanded={isExpanded}
                  >
                    {isExpanded ? "\u02C5" : "\u203A"}
                  </button>
                ) : (
                  <span className="tree-icon-box text-[10px] font-mono text-[var(--text-muted)] shrink-0">
                    {fileIcon(seg)}
                  </span>
                )}
                {isFile ? (
                  <>
                    <button
                      type="button"
                      className="tree-label text-left text-[var(--text)] hover:underline"
                      onClick={() => onOpenInEditor ? onOpenInEditor(child.fullPath!) : onOpenDiff(child.fullPath!, staged)}
                      title={child.fullPath}
                    >
                      {seg}
                    </button>
                    <button
                      type="button"
                      className="tree-icon-box text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10 rounded opacity-0 group-hover:opacity-100"
                      onClick={(e) => { e.stopPropagation(); onOpenDiff(child.fullPath!, staged); }}
                      title={t("viewDiff")}
                    >
                      {"\u2194"}
                    </button>
                    <span className={statusBadgeClass} title={status === "U" ? "Untracked" : status === "M" ? "Modified" : status === "D" ? "Deleted" : "Renamed"}>
                      {status}
                    </span>
                    {staged ? (
                      <button
                        type="button"
                        className="tree-icon-box text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10 rounded opacity-0 group-hover:opacity-100"
                        onClick={(e) => { e.stopPropagation(); onUnstage(child.fullPath!); }}
                        title={t("unstage")}
                      >
                        −
                      </button>
                    ) : (
                      <button
                        type="button"
                        className="tree-icon-box text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10 rounded opacity-0 group-hover:opacity-100"
                        onClick={(e) => { e.stopPropagation(); onStage(child.fullPath!); }}
                        title={t("stage")}
                      >
                        +
                      </button>
                    )}
                  </>
                ) : (
                  <span
                    className="tree-label text-[var(--text-muted)] cursor-pointer font-medium"
                    onClick={() => toggleExpanded(key)}
                  >
                    {seg}
                  </span>
                )}
              </div>
              {hasChildren && isExpanded && (
                <FileTree node={child} depth={depth + 1} pathPrefix={key} onStage={onStage} onUnstage={onUnstage} />
              )}
            </div>
          );
        })}
      </>
    );
  }

  const commitPlaceholder = t("commitMessageHint").replace("{branch}", status.branch);

  return (
    <div className="flex flex-col h-full min-h-0">
      <div className="shrink-0 flex items-center justify-between gap-2 px-2 py-1.5 border-b border-border">
        <span className="text-xs font-medium text-[var(--text-muted)] truncate" title={status.branch}>
          {status.branch}
        </span>
        <button
          type="button"
          className="shrink-0 text-xs text-[var(--text-muted)] hover:text-[var(--text)]"
          onClick={refresh}
          title={t("refresh")}
        >
          {t("refresh")}
        </button>
      </div>
      {error && (
        <div className="shrink-0 px-2 py-1 text-xs text-red-400">{error}</div>
      )}
      <div className="shrink-0 border-b border-border p-2 flex flex-col gap-1.5">
        <textarea
          className="w-full min-h-[3.5rem] px-2 py-1.5 text-sm bg-[#3c3c3c] border border-gray-600 rounded resize-none"
          placeholder={commitPlaceholder}
          value={commitMessage}
          onChange={(e) => setCommitMessage(e.target.value)}
          rows={2}
        />
        <button
          type="button"
          className="w-full py-1.5 text-sm bg-primary text-white rounded hover:opacity-90 disabled:opacity-50 flex items-center justify-center gap-1"
          onClick={handleCommit}
          disabled={!commitMessage.trim() || !hasStaged}
        >
          <span aria-hidden>{"\u2713"}</span>
          {t("commit")}
        </button>
      </div>
      <div className="flex-1 min-h-0 overflow-auto flex flex-col scroll-vscode scm-tree">
        <div className="shrink-0 border-b border-border">
          <button
            type="button"
            className="flex items-center gap-2 w-full px-2 py-1.5 text-left hover:bg-white/5"
            onClick={() => setStagedOpen((o) => !o)}
            aria-expanded={stagedOpen}
          >
            <span className="text-[var(--text-muted)] shrink-0">{stagedOpen ? "\u25BC" : "\u25B6"}</span>
            <span className="text-xs font-medium text-[var(--text)]">{t("staged")}</span>
            {stagedCount > 0 && (
              <span className="min-w-[1.25rem] h-4 px-1 flex items-center justify-center text-[10px] rounded bg-green-500/25 text-green-400">
                {stagedCount}
              </span>
            )}
          </button>
          {stagedOpen && (
            <div className="px-2 pb-2 pt-0">
              {stagedCount > 0 ? (
                <FileTree
                  node={stagedTree}
                  depth={0}
                  pathPrefix=""
                  onStage={() => {}}
                  onUnstage={(path) => handleUnstage([path])}
                />
              ) : (
                <div className="text-xs text-[var(--text-muted)] py-1">{t("noStaged")}</div>
              )}
            </div>
          )}
        </div>
        <div className="flex-1 min-h-0 flex flex-col">
          <button
            type="button"
            className="flex items-center gap-2 w-full px-2 py-1.5 text-left hover:bg-white/5 shrink-0"
            onClick={() => setChangesOpen((o) => !o)}
            aria-expanded={changesOpen}
          >
            <span className="text-[var(--text-muted)] shrink-0">{changesOpen ? "\u25BC" : "\u25B6"}</span>
            <span className="text-xs font-medium text-[var(--text)]">{t("changes")}</span>
            {changesCount > 0 && (
              <span className="min-w-[1.25rem] h-4 px-1 flex items-center justify-center text-[10px] rounded bg-amber-500/25 text-amber-400">
                {changesCount}
              </span>
            )}
          </button>
          {changesOpen && (
            <div className="px-2 pb-2 pt-0">
              {changesCount > 0 ? (
                <FileTree
                  node={changesTree}
                  depth={0}
                  pathPrefix=""
                  onStage={(path) => handleStage([path])}
                  onUnstage={() => {}}
                />
              ) : (
                <div className="text-xs text-[var(--text-muted)] py-1">{t("noChanges")}</div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
