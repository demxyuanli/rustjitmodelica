import type React from "react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { listInstantiableClasses } from "../api/tauri";
import type { InstantiableClass } from "../types";
import type { MoTreeEntry } from "../hooks/useProject";
import { recentProjectDisplayName } from "../hooks/useRecentProjects";
import { t } from "../i18n";
import { AppIcon } from "./Icon";
import { FileIcon } from "./FileIcon";
import { useDiagramScheme } from "../contexts/DiagramSchemeContext";

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

type CategoryKey = "all" | "electrical" | "mechanical" | "thermal" | "signal" | "math" | "other";

const CATEGORY_KEYWORDS: Record<CategoryKey, string[]> = {
  all: [],
  electrical: ["electric", "circuit", "resistor", "capacitor", "inductor", "voltage", "current", "diode", "transistor", "opamp"],
  mechanical: ["mechanic", "mass", "spring", "damper", "inertia", "torque", "force", "gear", "shaft", "bearing"],
  thermal: ["thermal", "heat", "temperature", "convect", "conduct", "radiation", "insulation"],
  signal: ["signal", "input", "output", "block", "gain", "integrator", "transfer", "pid", "controller", "feedback"],
  math: ["math", "sin", "cos", "sqrt", "abs", "exp", "log", "function", "constant", "table"],
  other: [],
};

function classifyComponent(item: InstantiableClass): CategoryKey {
  const text = `${item.qualifiedName} ${item.name} ${item.libraryName} ${item.summary ?? ""} ${item.kind}`.toLowerCase();
  for (const [cat, keywords] of Object.entries(CATEGORY_KEYWORDS) as [CategoryKey, string[]][]) {
    if (cat === "all" || cat === "other") continue;
    if (keywords.some((kw) => text.includes(kw))) return cat;
  }
  return "other";
}

const FAVORITES_KEY = "modai-library-favorites";

function loadFavorites(): Set<string> {
  try {
    const raw = localStorage.getItem(FAVORITES_KEY);
    return raw ? new Set(JSON.parse(raw) as string[]) : new Set();
  } catch { return new Set(); }
}

function saveFavorites(fav: Set<string>) {
  try {
    localStorage.setItem(FAVORITES_KEY, JSON.stringify([...fav]));
  } catch { /* ignore */ }
}

const TREE_INDENT = 14;
const TREE_BASE = 8;
const TREE_ICON = 16;

function TreeNode({
  entry,
  depth,
  onOpenFile,
  defaultExpanded,
  onFileContextMenu,
  onFolderContextMenu,
}: {
  entry: MoTreeEntry;
  depth: number;
  onOpenFile?: (path: string) => void;
  defaultExpanded: boolean;
  onFileContextMenu?: (info: { path: string; name: string; event: React.MouseEvent }) => void;
  onFolderContextMenu?: (info: { name: string; event: React.MouseEvent }) => void;
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
            onFileContextMenu={onFileContextMenu}
            onFolderContextMenu={onFolderContextMenu}
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
            onContextMenu={(event) => {
              if (!entry.path || !onFileContextMenu) return;
              event.preventDefault();
              onFileContextMenu({ path: entry.path, name: entry.name, event });
            }}
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
            onContextMenu={(event) => {
              if (!onFolderContextMenu) return;
              event.preventDefault();
              onFolderContextMenu({ name: entry.name, event });
            }}
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
              onFileContextMenu={onFileContextMenu}
              onFolderContextMenu={onFolderContextMenu}
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
  recentProjects?: string[];
  onOpenRecentProject?: (path: string) => void;
  onFileContextMenu?: (info: { path: string; name: string; event: React.MouseEvent }) => void;
  onFolderContextMenu?: (info: { name: string; event: React.MouseEvent }) => void;
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
  recentProjects = [],
  onOpenRecentProject,
  onFileContextMenu,
  onFolderContextMenu,
}: LibrariesBrowserProps) {
  const [query, setQuery] = useState("");
  const [classes, setClasses] = useState<InstantiableClass[]>([]);
  const [category, setCategory] = useState<CategoryKey>("all");
  const [favorites, setFavorites] = useState<Set<string>>(loadFavorites);
  const [showFavoritesOnly, setShowFavoritesOnly] = useState(false);
  const { scheme } = useDiagramScheme();
  const categoryColors = useMemo<Record<CategoryKey, string>>(
    () => ({
      all: "var(--text-muted)",
      electrical: scheme.connectorColors.electrical ?? "#2563eb",
      mechanical: scheme.connectorColors.mechanical ?? "#059669",
      thermal: scheme.connectorColors.thermal ?? "#dc2626",
      signal: scheme.connectorColors.signal_input ?? "#f59e0b",
      math: "#8b5cf6",
      other: "var(--text-muted)",
    }),
    [scheme]
  );

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

  const toggleFavorite = useCallback((qualifiedName: string) => {
    setFavorites((prev) => {
      const next = new Set(prev);
      if (next.has(qualifiedName)) next.delete(qualifiedName);
      else next.add(qualifiedName);
      saveFavorites(next);
      return next;
    });
  }, []);

  const filteredClasses = useMemo(() => {
    let result = classes;
    const term = query.trim().toLowerCase();
    if (term) {
      result = result.filter((item) =>
        item.name.toLowerCase().includes(term)
        || item.qualifiedName.toLowerCase().includes(term)
        || item.libraryName.toLowerCase().includes(term)
        || item.path?.toLowerCase().includes(term)
        || item.summary?.toLowerCase().includes(term)
        || item.usageHelp?.toLowerCase().includes(term)
        || item.exampleTitles?.some((title) => title.toLowerCase().includes(term))
      );
    }
    if (category !== "all") {
      result = result.filter((item) => classifyComponent(item) === category);
    }
    if (showFavoritesOnly) {
      result = result.filter((item) => favorites.has(item.qualifiedName));
    }
    return result;
  }, [classes, query, category, showFavoritesOnly, favorites]);

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

  const categories: { key: CategoryKey; label: string }[] = [
    { key: "all", label: t("allComponents") },
    { key: "electrical", label: t("categoryElectrical") },
    { key: "mechanical", label: t("categoryMechanical") },
    { key: "thermal", label: t("categoryThermal") },
    { key: "signal", label: t("categorySignal") },
    { key: "math", label: t("categoryMath") },
    { key: "other", label: t("categoryOther") },
  ];

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
          <>
            <input
              type="text"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t("tableSearch")}
              className="w-full rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1 text-xs text-[var(--text)]"
            />
            <div className="flex flex-wrap gap-1">
              {categories.map((cat) => (
                <button
                  key={cat.key}
                  type="button"
                  className={`px-1.5 py-0.5 rounded text-[9px] border transition-colors ${
                    category === cat.key
                      ? "border-primary bg-primary/15 text-[var(--text)]"
                      : "border-transparent text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/5"
                  }`}
                  onClick={() => setCategory(cat.key)}
                >
                  <span className="inline-block w-1.5 h-1.5 rounded-full mr-0.5" style={{ backgroundColor: categoryColors[cat.key] }} />
                  {cat.label}
                </button>
              ))}
            </div>
            <div className="flex items-center gap-2">
              <button
                type="button"
                className={`text-[10px] px-1.5 py-0.5 rounded ${showFavoritesOnly ? "bg-yellow-500/20 text-yellow-400" : "text-[var(--text-muted)] hover:text-[var(--text)]"}`}
                onClick={() => setShowFavoritesOnly(!showFavoritesOnly)}
              >
                {showFavoritesOnly ? "\u2605" : "\u2606"} {t("favorites")} ({favorites.size})
              </button>
              <span className="text-[10px] text-[var(--text-muted)]">
                {filteredClasses.length} / {classes.length}
              </span>
            </div>
          </>
        )}
      </div>

      <div className="flex-1 min-h-0 overflow-auto">
        {showProjectFiles && !projectDir && recentProjects.length > 0 && (
          <div className="p-2 border-b border-[var(--border)]">
            <div className="text-[10px] uppercase tracking-wide text-[var(--text-muted)] mb-1.5">
              {t("recentProjects")}
            </div>
            <ul className="text-xs space-y-0.5">
              {recentProjects.slice(0, 10).map((dir) => (
                <li key={dir}>
                  <button
                    type="button"
                    className="tree-row w-full text-left rounded px-1 py-1 hover:bg-white/10 text-[var(--text)] flex items-center gap-1.5"
                    title={dir}
                    onClick={() => onOpenRecentProject?.(dir)}
                  >
                    <AppIcon name="explorer" className="w-3.5 h-3.5 shrink-0 text-[var(--text-muted)]" aria-hidden />
                    <span className="truncate">{recentProjectDisplayName(dir)}</span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )}
        {projectDir ? (
          <>
            {showProjectFiles && (
              <div className="p-2">
                {showProjectTree ? (
                  <div className="text-xs">
                    <TreeNode
                      entry={moTree!}
                      depth={0}
                      onOpenFile={onOpenFile}
                      defaultExpanded={true}
                      onFileContextMenu={onFileContextMenu}
                      onFolderContextMenu={onFolderContextMenu}
                    />
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
                      {items.map((item) => {
                        const cat = classifyComponent(item);
                        const isFav = favorites.has(item.qualifiedName);
                        return (
                          <div
                            key={`${group}:${item.libraryId}:${item.qualifiedName}`}
                            className="px-2 py-1.5 rounded hover:bg-[var(--surface-hover)] text-xs text-[var(--text)] flex items-center gap-2 group/item"
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
                            <span
                              className="shrink-0 w-2 h-2 rounded-full"
                              style={{ backgroundColor: categoryColors[cat] }}
                              title={cat}
                            />
                            <button
                              type="button"
                              className="min-w-0 flex-1 text-left"
                              title={item.qualifiedName}
                              onClick={() => onOpenType?.(item.qualifiedName, item.libraryId)}
                            >
                              <div className="truncate">{item.name}</div>
                              <div className="truncate text-[10px] text-[var(--text-muted)]">{item.libraryName}</div>
                            </button>
                            <button
                              type="button"
                              className={`shrink-0 text-sm opacity-0 group-hover/item:opacity-100 transition-opacity ${isFav ? "text-yellow-400 !opacity-100" : "text-[var(--text-muted)]"}`}
                              onClick={(e) => { e.stopPropagation(); toggleFavorite(item.qualifiedName); }}
                              title={t("favorites")}
                            >
                              {isFav ? "\u2605" : "\u2606"}
                            </button>
                            <span className="shrink-0 text-[10px] text-[var(--text-muted)]">{item.kind}</span>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </>
        ) : null}
      </div>
    </aside>
  );
}
