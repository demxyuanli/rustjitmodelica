import { useState } from "react";
import { t } from "../i18n";
import type { MoTreeEntry } from "../App";

const TREE_INDENT = 14;
const TREE_BASE = 8;
const TREE_ICON = 16;

function fileIcon(name: string): string {
  const ext = name.includes(".") ? name.split(".").pop()?.toLowerCase() : "";
  if (ext === "tsx" || ext === "ts") return "T";
  if (ext === "json") return "J";
  if (ext === "mo") return "M";
  if (ext === "h" || ext === "c" || ext === "cpp" || ext === "rs") return "C";
  if (ext === "md" || ext === "txt") return "T";
  return "F";
}

interface FileTreeProps {
  projectDir: string | null;
  moTree: MoTreeEntry | null;
  moFiles: string[];
  onOpenProject: () => void;
  onOpenFile: (relativePath: string) => void;
}

function TreeNode({
  entry,
  depth,
  onOpenFile,
  defaultExpanded,
}: {
  entry: MoTreeEntry;
  depth: number;
  onOpenFile: (path: string) => void;
  defaultExpanded: boolean;
}) {
  const [expanded, setExpanded] = useState(defaultExpanded);
  const hasChildren = entry.children && entry.children.length > 0;
  const isFile = entry.path != null;

  const paddingLeft = TREE_BASE + depth * TREE_INDENT;

  if (entry.name === "" && entry.children) {
    return (
      <>
        {entry.children.map((child, i) => (
          <TreeNode
            key={child.path ?? child.name + String(i)}
            entry={child}
            depth={0}
            onOpenFile={onOpenFile}
            defaultExpanded={depth < 1}
          />
        ))}
      </>
    );
  }

  return (
    <div className="flex flex-col">
      <div className="tree-row group rounded" style={{ paddingLeft }}>
        {hasChildren ? (
          <button
            type="button"
            className="tree-arrow text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10 rounded"
            onClick={() => setExpanded((e) => !e)}
            aria-expanded={expanded}
          >
            {expanded ? "\u02C5" : "\u203A"}
          </button>
        ) : (
          <span className="tree-icon-box text-[10px] font-mono text-[var(--text-muted)] shrink-0">
            {fileIcon(entry.name)}
          </span>
        )}
        {isFile ? (
          <button
            type="button"
            className="tree-label text-left text-[var(--text)] hover:bg-white/10 rounded px-1"
            onClick={() => entry.path && onOpenFile(entry.path)}
            title={entry.path}
          >
            <span>{entry.name}</span>
            {entry.class_name && (
              <span className="text-[var(--text-muted)] ml-1 text-[10px]">
                ({entry.class_name})
              </span>
            )}
          </button>
        ) : (
          <span
            className="tree-label font-medium text-[var(--text-muted)] cursor-default px-1"
            onClick={() => hasChildren && setExpanded((e) => !e)}
          >
            {entry.name}
          </span>
        )}
      </div>
      {hasChildren && expanded && (
        <div className="flex flex-col">
          {entry.children!.map((child, i) => (
            <TreeNode
              key={child.path ?? child.name + String(i)}
              entry={child}
              depth={depth + 1}
              onOpenFile={onOpenFile}
              defaultExpanded={depth < 0}
            />
          ))}
        </div>
      )}
      {isFile && entry.extends && entry.extends.length > 0 && (
        <div
          className="text-[10px] text-[var(--text-muted)] border-l border-border/50 pl-2 py-0.5 mb-0.5"
          style={{ marginLeft: paddingLeft + TREE_ICON }}
        >
          <span className="font-medium">{t("extendsLabel")}:</span> {entry.extends.join(", ")}
        </div>
      )}
    </div>
  );
}

export function FileTree({
  projectDir,
  moTree,
  moFiles,
  onOpenProject,
  onOpenFile,
}: FileTreeProps) {
  const showTree = projectDir && moTree?.children && moTree.children.length > 0;

  return (
    <aside className="w-full bg-surface-alt p-2">
      <div className="flex items-center justify-between mb-2">
        <span className="text-sm font-medium text-[var(--text-muted)]">{t("project")}</span>
      </div>
      <button
        type="button"
        onClick={onOpenProject}
        className="w-full px-2 py-1.5 text-left text-sm rounded bg-primary/20 hover:bg-primary/30 text-primary mb-2"
      >
        {t("openProject")}
      </button>
      {projectDir ? (
        showTree ? (
          <div className="text-xs">
            <TreeNode
              entry={moTree!}
              depth={0}
              onOpenFile={onOpenFile}
              defaultExpanded={true}
            />
          </div>
        ) : (
          <ul className="text-xs space-y-0.5">
            {moFiles.map((f) => (
              <li key={f}>
                <button
                  type="button"
                  className="w-full text-left px-2 py-1 rounded hover:bg-white/10 truncate"
                  onClick={() => onOpenFile(f)}
                  title={f}
                >
                  {f.split(/[/\\]/).pop() ?? f}
                </button>
              </li>
            ))}
          </ul>
        )
      ) : (
        <div className="text-xs text-[var(--text-muted)]">{t("noProjectOpen")}</div>
      )}
    </aside>
  );
}
