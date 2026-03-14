import { useEffect, useMemo, useState } from "react";
import { listInstantiableClasses } from "../api/tauri";
import type { InstantiableClass } from "../types";
import type { MoTreeEntry } from "../hooks/useProject";
import { t } from "../i18n";
import { FileIcon } from "./FileIcon";

export const MODELICA_DRAG_TYPE = "application/modelica-type";

export interface ModelicaDragPayload {
  typeName: string;
  displayName: string;
  source: string;
  libraryId?: string;
  libraryName?: string;
  path?: string;
}

export function decodeModelicaDragPayload(raw: string): ModelicaDragPayload | null {
  try {
    const parsed = JSON.parse(raw) as Partial<ModelicaDragPayload>;
    if (!parsed || typeof parsed.typeName !== "string" || typeof parsed.displayName !== "string" || typeof parsed.source !== "string") {
      return null;
    }
    return {
      typeName: parsed.typeName,
      displayName: parsed.displayName,
      source: parsed.source,
      libraryId: typeof parsed.libraryId === "string" ? parsed.libraryId : undefined,
      libraryName: typeof parsed.libraryName === "string" ? parsed.libraryName : undefined,
      path: typeof parsed.path === "string" ? parsed.path : undefined,
    };
  } catch {
    return null;
  }
}

const TREE_INDENT = 14;
const TREE_BASE = 8;
const TREE_ICON = 16;

function TreeNode({
  entry,
  depth,
  onOpenFile,
  defaultExpanded,
}: {
  entry: MoTreeEntry;
  depth: number;
  onOpenFile?: (path: string) => void;
  defaultExpanded: boolean;
}) {
  const [expanded, setExpanded] = useState(defaultExpanded);
  const hasChildren = entry.children && entry.children.length > 0;
  const isFile = entry.path != null;
  const paddingLeft = TREE_BASE + depth * TREE_INDENT;

  if (entry.name === "" && entry.children) {
    return (
      <>
        {entry.children.map((child, index) => (
          <TreeNode
            key={child.path ?? child.name + String(index)}
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
            onClick={() => setExpanded((value) => !value)}
            aria-expanded={expanded}
          >
            {expanded ? "\u02C5" : "\u203A"}
          </button>
        ) : (
          <span className="tree-icon-box shrink-0">
            <FileIcon name={entry.name} />
          </span>
        )}
        {isFile ? (
          <button
            type="button"
            className="tree-label text-left text-[var(--text)] hover:bg-white/10 rounded px-1"
            onClick={() => entry.path && onOpenFile?.(entry.path)}
            title={entry.path ?? undefined}
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
            onClick={() => hasChildren && setExpanded((value) => !value)}
          >
            {entry.name}
          </span>
        )}
      </div>
      {hasChildren && expanded && (
        <div className="flex flex-col">
          {entry.children!.map((child, index) => (
            <TreeNode
              key={child.path ?? child.name + String(index)}
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

export interface LibrariesBrowserProps {
  projectDir: string | null;
  moTree?: MoTreeEntry | null;
  moFiles?: string[];
  readOnly?: boolean;
  variant?: "standalone" | "embedded";
  libraryRefreshToken?: number;
  onOpenProject?: () => void;
  onOpenFile?: (relativePath: string) => void;
  onOpenType?: (typeName: string, libraryId?: string) => void;
}

export function LibrariesBrowser({
  projectDir,
  moTree = null,
  moFiles = [],
  readOnly = false,
  variant = "standalone",
  libraryRefreshToken = 0,
  onOpenProject,
  onOpenFile,
  onOpenType,
}: LibrariesBrowserProps) {
  const [query, setQuery] = useState("");
  const [classes, setClasses] = useState<InstantiableClass[]>([]);

  useEffect(() => {
    if (!projectDir || variant !== "standalone") {
      setClasses([]);
      return;
    }
    let cancelled = false;
    listInstantiableClasses(projectDir)
      .then((items) => {
        if (!cancelled) {
          setClasses(items);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setClasses([]);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [projectDir, variant, libraryRefreshToken]);

  const filteredClasses = useMemo(() => {
    const term = query.trim().toLowerCase();
    if (!term) {
      return classes;
    }
    return classes.filter((item) =>
      item.name.toLowerCase().includes(term)
      || item.qualifiedName.toLowerCase().includes(term)
      || item.libraryName.toLowerCase().includes(term)
      || item.path?.toLowerCase().includes(term)
      || item.summary?.toLowerCase().includes(term)
      || item.usageHelp?.toLowerCase().includes(term)
      || item.exampleTitles?.some((title) => title.toLowerCase().includes(term))
    );
  }, [classes, query]);

  const groupedClasses = useMemo(() => {
    const groups = new Map<string, InstantiableClass[]>();
    for (const item of filteredClasses) {
      const bucket = groups.get(item.source) ?? [];
      bucket.push(item);
      groups.set(item.source, bucket);
    }
    return Array.from(groups.entries());
  }, [filteredClasses]);

  const showProjectTree = Boolean(projectDir && moTree?.children && moTree.children.length > 0);
  const containerClass =
    variant === "embedded"
      ? "w-full min-w-0 bg-[var(--bg-elevated)] flex flex-col min-h-0 border-b border-[var(--border)]"
      : "w-72 shrink-0 border-r border-[var(--border)] bg-[var(--bg-elevated)] flex flex-col min-h-0";
  const browserTitle = variant === "embedded" ? t("projectFiles") : t("availableComponentLibrary");
  const showLibraryGroups = variant === "standalone";
  const showProjectFiles = variant === "embedded";
  const groupTitle = (group: string) => {
    switch (group) {
      case "system":
        return t("componentLibrariesSystem");
      case "global":
        return t("componentLibrariesGlobal");
      case "project":
        return t("componentLibrariesProject");
      default:
        return group;
    }
  };

  return (
    <aside className={containerClass}>
      <div className="p-2 border-b border-[var(--border)] space-y-2">
        <div className="flex items-center justify-between gap-2">
          <div className="text-xs font-medium uppercase tracking-wide text-[var(--text-muted)]">
            {browserTitle}
          </div>
          {onOpenProject && (
            <button
              type="button"
              onClick={onOpenProject}
              className="rounded bg-primary/20 px-2 py-1 text-[10px] text-primary hover:bg-primary/30"
            >
              {t("openProject")}
            </button>
          )}
        </div>
        {showLibraryGroups && (
          <input
            type="text"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t("tableSearch")}
            className="w-full rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1 text-xs text-[var(--text)]"
          />
        )}
      </div>

      <div className="flex-1 min-h-0 overflow-auto">
        {projectDir ? (
          <>
            {showProjectFiles && (
              <div className="p-2">
                {showProjectTree ? (
                  <div className="text-xs">
                    <TreeNode entry={moTree!} depth={0} onOpenFile={onOpenFile} defaultExpanded={true} />
                  </div>
                ) : (
                  <ul className="text-xs space-y-0.5">
                    {moFiles.map((filePath) => (
                      <li key={filePath}>
                        <button
                          type="button"
                          className="w-full text-left px-2 py-1 rounded hover:bg-[var(--surface-hover)] truncate"
                          onClick={() => onOpenFile?.(filePath)}
                          title={filePath}
                        >
                          {filePath.split(/[/\\]/).pop() ?? filePath}
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )}

            {showLibraryGroups && (
              <div className="p-2 space-y-3">
                {groupedClasses.map(([group, items]) => (
                  <div key={group}>
                    <div className="mb-1 px-2 text-[10px] uppercase tracking-wide text-[var(--text-muted)]">
                      {groupTitle(group)}
                    </div>
                    <div className="space-y-0.5">
                      {items.map((item) => (
                        <div
                          key={`${group}:${item.libraryId}:${item.qualifiedName}`}
                          className="px-2 py-1.5 rounded hover:bg-[var(--surface-hover)] text-xs text-[var(--text)] flex items-center justify-between gap-2"
                          draggable={!readOnly}
                          onDragStart={(event) => {
                            if (readOnly) return;
                            const payload: ModelicaDragPayload = {
                              typeName: item.qualifiedName,
                              displayName: item.name,
                              source: item.source,
                              libraryId: item.libraryId,
                              libraryName: item.libraryName,
                              path: item.path,
                            };
                            event.dataTransfer.setData(MODELICA_DRAG_TYPE, JSON.stringify(payload));
                            event.dataTransfer.effectAllowed = "copy";
                          }}
                        >
                          <button
                            type="button"
                            className="min-w-0 flex-1 text-left"
                            title={item.qualifiedName}
                            onClick={() => onOpenType?.(item.qualifiedName, item.libraryId)}
                          >
                            <div className="truncate">{item.name}</div>
                            <div className="truncate text-[10px] text-[var(--text-muted)]">{item.libraryName}</div>
                            <div className="truncate text-[10px] text-[var(--text-muted)]">{item.qualifiedName}</div>
                          </button>
                          <span className="shrink-0 text-[10px] text-[var(--text-muted)]">{item.kind}</span>
                        </div>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </>
        ) : (
          <div className="p-3 text-xs text-[var(--text-muted)]">{t("noProjectOpen")}</div>
        )}
      </div>
    </aside>
  );
}
