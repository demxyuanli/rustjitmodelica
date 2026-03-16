import { useState, useEffect } from "react";
import { t } from "../i18n";
import { useGit, type GitStatus } from "../hooks/useGit";
import { FileIcon } from "./FileIcon";
import { AppIcon } from "./Icon";
import { IconButton } from "./IconButton";
import { ContextMenu } from "./ContextMenu";

export type { GitStatus };
export type { GitLogEntry, GitCommitFile } from "../hooks/useGit";

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
  const git = useGit(projectDir, onRefreshStatus);

  const [expandedDirs, setExpandedDirs] = useState<Set<string>>(new Set());
  const [stagedOpen, setStagedOpen] = useState(true);
  const [changesOpen, setChangesOpen] = useState(true);
  const [menuVisible, setMenuVisible] = useState(false);
  const [menuPosition, setMenuPosition] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const [menuTarget, setMenuTarget] = useState<{ path: string; isStaged: boolean } | null>(null);

  useEffect(() => {
    if (!git.status) return;
    const st = git.status;
    const paths = [
      ...(st.staged ?? []),
      ...(st.modified ?? []),
      ...(st.deleted ?? []),
      ...(st.untracked ?? []),
      ...(st.renamed ?? []).map((r) => r.to),
    ];
    const prefixes = new Set<string>();
    paths.forEach((path) => {
      const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
      for (let i = 1; i < parts.length; i++) {
        prefixes.add(parts.slice(0, i).join("/"));
      }
    });
    setExpandedDirs(prefixes);
  }, [git.status]);

  if (!projectDir) {
    return (
      <div className="p-3 text-sm text-[var(--text-muted)]">
        {t("openProjectFirst")}
      </div>
    );
  }

  if (git.loading && !git.status) {
    return (
      <div className="p-3 text-sm text-[var(--text-muted)]">{t("running")}</div>
    );
  }

  if (!git.isRepo || !git.status) {
    return (
      <div className="p-3 flex flex-col gap-2">
        <p className="text-sm text-[var(--text-muted)]">{git.error || t("notGitRepo")}</p>
        <button
          type="button"
          className="self-start px-3 py-1.5 text-sm bg-primary text-white rounded hover:opacity-90 disabled:opacity-50"
          onClick={git.initRepo}
          disabled={git.initLoading}
        >
          {git.initLoading ? t("running") : t("initGitRepo")}
        </button>
      </div>
    );
  }

  const status = git.status;
  const staged = status.staged ?? [];
  const modified = status.modified ?? [];
  const deleted = status.deleted ?? [];
  const untracked = status.untracked ?? [];
  const renamed = status.renamed ?? [];

  const stagedPaths: { path: string; status: string; isStaged: boolean }[] = [
    ...staged.map((path) => ({ path, status: "M", isStaged: true })),
    ...renamed.map((r) => ({ path: r.to, status: "R", isStaged: true })),
  ];
  const unstagedPaths: { path: string; status: string; isStaged: boolean }[] = [];
  modified.forEach((path) => {
    if (!staged.includes(path) && !renamed.some((r) => r.to === path)) {
      unstagedPaths.push({ path, status: "M", isStaged: false });
    }
  });
  deleted.forEach((path) => {
    if (!staged.includes(path)) {
      unstagedPaths.push({ path, status: "D", isStaged: false });
    }
  });
  untracked.forEach((path) => {
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
          const st = child.status ?? "M";
          const staged = child.isStaged ?? false;
          const statusBadgeClass = staged ? "scm-status-badge staged" : `scm-status-badge ${st === "M" ? "modified" : st === "U" ? "untracked" : st === "D" ? "deleted" : st === "R" ? "renamed" : "added"}`;
          return (
            <div key={key} className="flex flex-col">
              <div
                className={`tree-row group rounded ${isFile ? statusRowClass(st, staged) : ""}`}
                style={{ paddingLeft }}
                onContextMenu={(event) => {
                  if (!isFile || !child.fullPath) return;
                  event.preventDefault();
                  setMenuPosition({ x: event.clientX, y: event.clientY });
                  setMenuTarget({ path: child.fullPath, isStaged: staged });
                  setMenuVisible(true);
                }}
              >
                {hasChildren ? (
                  <IconButton
                    icon={<span aria-hidden>{isExpanded ? "\u02C5" : "\u203A"}</span>}
                    size="xs"
                    className="tree-arrow text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10 rounded"
                    onClick={() => toggleExpanded(key)}
                    aria-expanded={isExpanded}
                    aria-label={seg}
                    title={seg}
                  />
                ) : (
                  <span className="tree-icon-box shrink-0">
                    <FileIcon name={seg} />
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
                    <IconButton
                      icon={<AppIcon name="diff" aria-hidden="true" />}
                      size="xs"
                      className="tree-icon-box text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10 rounded opacity-0 group-hover:opacity-100"
                      onClick={(e) => {
                        e.stopPropagation();
                        onOpenDiff(child.fullPath!, staged);
                      }}
                      title={t("viewDiff")}
                      aria-label={t("viewDiff")}
                    />
                    <span className={statusBadgeClass} title={st === "U" ? "Untracked" : st === "M" ? "Modified" : st === "D" ? "Deleted" : "Renamed"}>
                      {st}
                    </span>
                    {staged ? (
                      <IconButton
                        icon={<AppIcon name="unstage" aria-hidden="true" />}
                        size="xs"
                        className="tree-icon-box text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10 rounded opacity-0 group-hover:opacity-100"
                        onClick={(e) => {
                          e.stopPropagation();
                          onUnstage(child.fullPath!);
                        }}
                        title={t("unstage")}
                        aria-label={t("unstage")}
                      />
                    ) : (
                      <IconButton
                        icon={<AppIcon name="stage" aria-hidden="true" />}
                        size="xs"
                        className="tree-icon-box text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10 rounded opacity-0 group-hover:opacity-100"
                        onClick={(e) => {
                          e.stopPropagation();
                          onStage(child.fullPath!);
                        }}
                        title={t("stage")}
                        aria-label={t("stage")}
                      />
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

  const commitPlaceholder = t("commitMessageHint").replace("{branch}", status.branch ?? "");

  return (
    <div className="flex flex-col h-full min-h-0">
      <div className="shrink-0 flex items-center justify-between gap-2 px-2 py-1.5 border-b border-border">
        <span className="text-xs font-medium text-[var(--text-muted)] truncate" title={status.branch}>
          {status.branch}
        </span>
        <IconButton
          icon={<AppIcon name="refresh" aria-hidden="true" />}
          size="xs"
          className="shrink-0 text-[var(--text-muted)] hover:text-[var(--text)]"
          onClick={git.refresh}
          title={t("refresh")}
          aria-label={t("refresh")}
        />
      </div>
      {git.error && (
        <div className="shrink-0 px-2 py-1 text-xs text-[var(--danger-text)]">{git.error}</div>
      )}
      <div className="shrink-0 border-b border-border p-2 flex flex-col gap-1.5">
        <textarea
          className="w-full min-h-[3.5rem] px-2 py-1.5 text-sm theme-input border rounded resize-none"
          placeholder={commitPlaceholder}
          value={git.commitMessage}
          onChange={(e) => git.setCommitMessage(e.target.value)}
          rows={2}
        />
        <button
          type="button"
          className="w-full py-1.5 text-sm bg-primary text-white rounded hover:opacity-90 disabled:opacity-50 flex items-center justify-center gap-1"
          onClick={git.commit}
          disabled={!git.commitMessage.trim() || !hasStaged}
        >
          <AppIcon name="gitCommit" aria-hidden="true" />
          <span className="sr-only">{t("commit")}</span>
        </button>
      </div>
      <div className="flex-1 min-h-0 overflow-auto flex flex-col scroll-vscode scm-tree">
        <div className="shrink-0 border-b border-border">
          <button
            type="button"
            className="flex items-center gap-2 w-full px-2 py-1.5 text-left hover:bg-[var(--surface-hover)]"
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
                  onUnstage={(path) => git.unstage([path])}
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
                  onStage={(path) => git.stage([path])}
                  onUnstage={() => {}}
                />
              ) : (
                <div className="text-xs text-[var(--text-muted)] py-1">{t("noChanges")}</div>
              )}
            </div>
          )}
        </div>
      </div>
      <ContextMenu
        visible={menuVisible}
        x={menuPosition.x}
        y={menuPosition.y}
        onClose={() => setMenuVisible(false)}
        items={
          menuTarget
            ? [
                {
                  id: "open-diff",
                  label: t("viewDiff"),
                  onClick: () => {
                    onOpenDiff(menuTarget.path, menuTarget.isStaged);
                  },
                },
                {
                  id: "open-editor",
                  label: t("openInEditor"),
                  onClick: () => {
                    onOpenInEditor?.(menuTarget.path);
                  },
                },
                !menuTarget.isStaged
                  ? {
                      id: "stage",
                      label: t("stage"),
                      onClick: () => {
                        git.stage([menuTarget.path]);
                      },
                    }
                  : {
                      id: "unstage",
                      label: t("unstage"),
                      onClick: () => {
                        git.unstage([menuTarget.path]);
                      },
                    },
                {
                  id: "discard",
                  label: t("contextDiscardChanges"),
                  disabled: menuTarget.isStaged,
                  onClick: () => {
                    if (menuTarget.isStaged) return;
                    // discard logic to be implemented with git helper if available
                  },
                },
              ]
            : []
        }
      />
    </div>
  );
}
