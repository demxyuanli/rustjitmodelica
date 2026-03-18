import { useCallback, useEffect, useMemo, useState } from "react";
import Editor from "@monaco-editor/react";
import { ChevronDown, ChevronRight } from "lucide-react";
import {
  addComponentLibrary,
  getComponentTypeDetails,
  installThirdPartyLibraryFromGit,
  listComponentLibraries,
  pickComponentLibraryFiles,
  pickComponentLibraryFolder,
  queryComponentLibraryTypes,
  readComponentTypeSource,
  removeComponentLibrary,
  setComponentLibraryEnabled,
  syncAllThirdPartyLibraries,
  syncThirdPartyLibrary,
} from "../api/tauri";
import { t, tf } from "../i18n";
import type {
  ComponentLibrary,
  ComponentTypeInfo,
  ComponentTypeSource,
  InstantiableClass,
} from "../types";
import { LibraryRelationGraphPane } from "./LibraryRelationGraphPane";
import { LibrarySourceSidebar } from "./LibrarySourceSidebar";
import { ContextMenu } from "./ContextMenu";

interface TypeTreeNode {
  segment: string;
  fullPath: string;
  children: Record<string, TypeTreeNode>;
  items: InstantiableClass[];
}

function buildTypeTree(items: InstantiableClass[]): TypeTreeNode {
  const root: TypeTreeNode = { segment: "", fullPath: "", children: {}, items: [] };
  for (const item of items) {
    const parts = item.qualifiedName.split(".");
    let current = root;
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      const fullPath = parts.slice(0, i + 1).join(".");
      if (!current.children[part]) {
        current.children[part] = { segment: part, fullPath, children: {}, items: [] };
      }
      current = current.children[part];
    }
    current.items.push(item);
  }
  return root;
}

function sortedChildEntries(node: TypeTreeNode): [string, TypeTreeNode][] {
  return Object.entries(node.children).sort(([a], [b]) => a.localeCompare(b, undefined, { sensitivity: "base" }));
}

const SCOPE_ICON: Record<string, { letter: string; bg: string }> = {
  system: { letter: "S", bg: "bg-blue-500/85" },
  global: { letter: "G", bg: "bg-emerald-500/85" },
  project: { letter: "P", bg: "bg-amber-500/85" },
};

function scopeIconTitle(scope: string): string {
  switch (scope) {
    case "system":
      return t("componentLibrariesSystem");
    case "global":
      return t("componentLibrariesGlobal");
    case "project":
      return t("componentLibrariesProject");
    default:
      return scope;
  }
}

function ScopeIcon({ scope }: { scope: string }) {
  const config = SCOPE_ICON[scope] ?? {
    letter: scope.charAt(0).toUpperCase(),
    bg: "bg-[var(--surface-hover)]",
  };
  return (
    <span
      className={`inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-[10px] font-semibold text-white ${config.bg}`}
      title={scopeIconTitle(scope)}
      aria-label={scopeIconTitle(scope)}
    >
      {config.letter}
    </span>
  );
}

function SectionIcon({ letter, bg, title }: { letter: string; bg: string; title: string }) {
  return (
    <span
      className={`inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-[10px] font-semibold text-white ${bg}`}
      title={title}
      aria-hidden="true"
    >
      {letter}
    </span>
  );
}

const TREE_INDENT_PX = 16;

function TypeTreeLevel({
  node,
  depth,
  expandedPaths,
  toggleExpanded,
  selectedKey,
  onSelect,
  componentKey: keyOf,
}: {
  node: TypeTreeNode;
  depth: number;
  expandedPaths: Set<string>;
  toggleExpanded: (fullPath: string) => void;
  selectedKey: string | null;
  onSelect: (key: string) => void;
  componentKey: (item: InstantiableClass) => string;
}) {
  const hasChildren = sortedChildEntries(node).length > 0;
  const isExpanded = expandedPaths.has(node.fullPath);
  const indent = depth * TREE_INDENT_PX;

  return (
    <div className="flex flex-col">
      {node.items.length > 0 &&
        node.items.map((item) => {
          const active = keyOf(item) === selectedKey;
          return (
            <button
              key={keyOf(item)}
              type="button"
              className={`w-full px-4 py-3 text-left transition-colors ${
                active ? "bg-[var(--surface-active)]" : "hover:bg-[var(--surface-hover)]"
              }`}
              style={{ paddingLeft: indent + 16 }}
              onClick={() => onSelect(keyOf(item))}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium text-[var(--text)]">{item.qualifiedName}</div>
                  <div className="mt-1 truncate text-[11px] text-[var(--text-muted)]">
                    {item.libraryName} · {item.kind}
                  </div>
                  {item.summary && (
                    <div className="mt-2 line-clamp-2 text-xs text-[var(--text-muted)]">{item.summary}</div>
                  )}
                </div>
                <ScopeIcon scope={item.libraryScope} />
              </div>
            </button>
          );
        })}
      {hasChildren && (
        <>
          <button
            type="button"
            className="flex w-full items-center gap-1 px-4 py-2 text-left text-sm text-[var(--text)] hover:bg-[var(--surface-hover)]"
            style={{ paddingLeft: indent + 16 }}
            onClick={() => toggleExpanded(node.fullPath)}
          >
            {isExpanded ? (
              <ChevronDown className="h-4 w-4 shrink-0" />
            ) : (
              <ChevronRight className="h-4 w-4 shrink-0" />
            )}
            <span className="truncate font-medium">{node.segment}</span>
          </button>
          {isExpanded &&
            sortedChildEntries(node).map(([, child]) => (
              <TypeTreeLevel
                key={child.fullPath}
                node={child}
                depth={depth + 1}
                expandedPaths={expandedPaths}
                toggleExpanded={toggleExpanded}
                selectedKey={selectedKey}
                onSelect={onSelect}
                componentKey={keyOf}
              />
            ))}
        </>
      )}
    </div>
  );
}

interface ComponentLibraryWorkspaceProps {
  projectDir?: string | null;
  /** When false, defer list/query until true to avoid startup cost when workspace is hidden */
  isActive?: boolean;
  theme: "dark" | "light";
  onLibrariesChanged?: () => void;
  onOpenType?: (typeName: string, libraryId?: string) => void;
}

type LibraryScopeFilter = "all" | "system" | "global" | "project";

const PAGE_SIZE = 120;

function componentKey(item: Pick<InstantiableClass, "qualifiedName" | "libraryId">) {
  return `${item.libraryId}::${item.qualifiedName}`;
}

export function ComponentLibraryWorkspace({
  projectDir = null,
  isActive = true,
  theme,
  onLibrariesChanged,
  onOpenType,
}: ComponentLibraryWorkspaceProps) {
  useEffect(() => {
    const start = performance.now?.() ?? Date.now();
    return () => {
      const end = performance.now?.() ?? Date.now();
      // eslint-disable-next-line no-console
      console.log("[modai-prof] ComponentLibraryWorkspace session took", end - start, "ms");
    };
  }, []);
  const [libraries, setLibraries] = useState<ComponentLibrary[]>([]);
  const [selectedLibraryId, setSelectedLibraryId] = useState<string>("all");
  const [scopeFilter, setScopeFilter] = useState<LibraryScopeFilter>("all");
  const [enabledOnly, setEnabledOnly] = useState(true);
  const [typeQuery, setTypeQuery] = useState("");
  const [debouncedTypeQuery, setDebouncedTypeQuery] = useState("");
  const [libraryQuery, setLibraryQuery] = useState("");
  const [libraryEnabledOnly, setLibraryEnabledOnly] = useState(false);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [typeItems, setTypeItems] = useState<InstantiableClass[]>([]);
  const [typeTotal, setTypeTotal] = useState(0);
  const [typeHasMore, setTypeHasMore] = useState(false);
  const [typesBusy, setTypesBusy] = useState(false);
  const [detail, setDetail] = useState<ComponentTypeInfo | null>(null);
  const [source, setSource] = useState<ComponentTypeSource | null>(null);
  const [busy, setBusy] = useState(false);
  const [detailBusy, setDetailBusy] = useState(false);
  const [banner, setBanner] = useState<string | null>(null);
  const [installFromUrlOpen, setInstallFromUrlOpen] = useState(false);
  const [installUrl, setInstallUrl] = useState("");
  const [installRef, setInstallRef] = useState("");
  const [installName, setInstallName] = useState("");
  const [installScope, setInstallScope] = useState<"global" | "project">("global");
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());
  const [detailsMenuVisible, setDetailsMenuVisible] = useState(false);
  const [detailsMenuPosition, setDetailsMenuPosition] = useState<{ x: number; y: number }>({ x: 0, y: 0 });

  const selectedLibrary = useMemo(
    () => libraries.find((item) => item.id === selectedLibraryId) ?? null,
    [libraries, selectedLibraryId]
  );

  useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedTypeQuery(typeQuery.trim()), 180);
    return () => window.clearTimeout(timer);
  }, [typeQuery]);

  useEffect(() => {
    if (!banner) {
      return;
    }
    const timer = window.setTimeout(() => setBanner(null), 3200);
    return () => window.clearTimeout(timer);
  }, [banner]);

  const loadLibraries = useCallback(async () => {
    const start = performance.now?.() ?? Date.now();
    try {
      const items = await listComponentLibraries(projectDir);
      setLibraries(items);
    } catch (error) {
      setLibraries([]);
      setBanner(String(error));
    } finally {
      const end = performance.now?.() ?? Date.now();
      // eslint-disable-next-line no-console
      console.log("[modai-prof] listComponentLibraries took", end - start, "ms");
    }
  }, [projectDir]);

  useEffect(() => {
    if (selectedLibraryId !== "all" && !libraries.some((item) => item.id === selectedLibraryId)) {
      setSelectedLibraryId("all");
    }
  }, [libraries, selectedLibraryId]);

  const loadTypePage = useCallback(
    async (offset: number, append: boolean) => {
      setTypesBusy(true);
      const start = performance.now?.() ?? Date.now();
      try {
        const result = await queryComponentLibraryTypes({
          projectDir,
          libraryId: selectedLibraryId === "all" ? undefined : selectedLibraryId,
          scope: scopeFilter === "all" ? undefined : scopeFilter,
          enabledOnly,
          query: debouncedTypeQuery,
          offset,
          limit: PAGE_SIZE,
        });
        setTypeItems((prev) => (append ? [...prev, ...result.items] : result.items));
        setTypeTotal(result.total);
        setTypeHasMore(result.hasMore);
      } catch (error) {
        if (!append) {
          setTypeItems([]);
          setTypeTotal(0);
          setTypeHasMore(false);
        }
        setBanner(String(error));
      } finally {
        setTypesBusy(false);
        const end = performance.now?.() ?? Date.now();
        // eslint-disable-next-line no-console
        console.log(
          "[modai-prof] queryComponentLibraryTypes took",
          end - start,
          "ms, offset=",
          offset,
          "append=",
          append
        );
      }
    },
    [debouncedTypeQuery, enabledOnly, projectDir, scopeFilter, selectedLibraryId]
  );

  useEffect(() => {
    if (isActive) void loadLibraries();
  }, [isActive, loadLibraries]);

  useEffect(() => {
    if (!isActive) return;
    setSelectedKey(null);
    setDetail(null);
    setSource(null);
    void loadTypePage(0, false);
  }, [isActive, loadTypePage]);

  const refreshWorkspace = useCallback(async () => {
    await loadLibraries();
    await loadTypePage(0, false);
    onLibrariesChanged?.();
  }, [loadLibraries, loadTypePage, onLibrariesChanged]);

  const typeTree = useMemo(() => buildTypeTree(typeItems), [typeItems]);

  useEffect(() => {
    if (typeItems.length === 0) {
      setSelectedKey(null);
      return;
    }
    if (!selectedKey || !typeItems.some((item) => componentKey(item) === selectedKey)) {
      setSelectedKey(componentKey(typeItems[0]));
    }
  }, [selectedKey, typeItems]);

  useEffect(() => {
    const firstLevel = sortedChildEntries(typeTree).map(([, child]) => child.fullPath);
    setExpandedPaths(new Set(firstLevel));
  }, [typeTree]);

  const toggleExpanded = useCallback((fullPath: string) => {
    setExpandedPaths((prev) => {
      const next = new Set(prev);
      if (next.has(fullPath)) next.delete(fullPath);
      else next.add(fullPath);
      return next;
    });
  }, []);

  const selectedClass = useMemo(
    () => typeItems.find((item) => componentKey(item) === selectedKey) ?? null,
    [selectedKey, typeItems]
  );

  useEffect(() => {
    if (!selectedClass) {
      setDetail(null);
      setSource(null);
      return;
    }
    let cancelled = false;
    setDetailBusy(true);
    Promise.all([
      getComponentTypeDetails(projectDir, selectedClass.qualifiedName, selectedClass.libraryId),
      readComponentTypeSource(projectDir, selectedClass.qualifiedName, selectedClass.libraryId),
    ])
      .then(([detailValue, sourceValue]) => {
        if (cancelled) {
          return;
        }
        setDetail(detailValue);
        setSource(sourceValue);
      })
      .catch((error) => {
        if (cancelled) {
          return;
        }
        setDetail(null);
        setSource(null);
        setBanner(String(error));
      })
      .finally(() => {
        if (!cancelled) {
          setDetailBusy(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [projectDir, selectedClass]);

  const handleImportLibraryFolder = useCallback(
    async (scope: "global" | "project") => {
      if (scope === "project" && !projectDir) {
        setBanner(t("componentLibrariesProjectUnavailable"));
        return;
      }
      setBusy(true);
      try {
        const selectedPath = await pickComponentLibraryFolder();
        if (!selectedPath) {
          return;
        }
        await addComponentLibrary({ projectDir, scope, kind: "folder", sourcePath: selectedPath });
        await refreshWorkspace();
        setBanner(t("componentLibraryImported"));
      } catch (error) {
        setBanner(String(error));
      } finally {
        setBusy(false);
      }
    },
    [projectDir, refreshWorkspace]
  );

  const handleImportLibraryFiles = useCallback(
    async (scope: "global" | "project") => {
      if (scope === "project" && !projectDir) {
        setBanner(t("componentLibrariesProjectUnavailable"));
        return;
      }
      setBusy(true);
      try {
        const selectedPaths = await pickComponentLibraryFiles();
        if (selectedPaths.length === 0) {
          return;
        }
        for (const sourcePath of selectedPaths) {
          await addComponentLibrary({ projectDir, scope, kind: "file", sourcePath });
        }
        await refreshWorkspace();
        setBanner(t("componentLibraryImported"));
      } catch (error) {
        setBanner(String(error));
      } finally {
        setBusy(false);
      }
    },
    [projectDir, refreshWorkspace]
  );

  const handleToggleLibrary = useCallback(
    async (library: ComponentLibrary) => {
      if (library.builtIn) {
        return;
      }
      setBusy(true);
      try {
        await setComponentLibraryEnabled({
          projectDir,
          scope: library.scope,
          libraryId: library.id,
          enabled: !library.enabled,
        });
        await refreshWorkspace();
      } catch (error) {
        setBanner(String(error));
      } finally {
        setBusy(false);
      }
    },
    [projectDir, refreshWorkspace]
  );

  const handleRemoveLibrary = useCallback(
    async (library: ComponentLibrary) => {
      if (library.builtIn) {
        return;
      }
      setBusy(true);
      try {
        await removeComponentLibrary({
          projectDir,
          scope: library.scope,
          libraryId: library.id,
        });
        await refreshWorkspace();
        setBanner(t("componentLibraryRemoved"));
      } catch (error) {
        setBanner(String(error));
      } finally {
        setBusy(false);
      }
    },
    [projectDir, refreshWorkspace]
  );

  const handleInstallFromUrl = useCallback(
    async () => {
      const url = installUrl.trim();
      if (!url) {
        setBanner("URL is required");
        return;
      }
      if (installScope === "project" && !projectDir) {
        setBanner(t("componentLibrariesProjectUnavailable"));
        return;
      }
      setBusy(true);
      try {
        await installThirdPartyLibraryFromGit({
          projectDir,
          scope: installScope,
          url,
          refName: installRef.trim() || undefined,
          displayName: installName.trim() || undefined,
        });
        setInstallFromUrlOpen(false);
        setInstallUrl("");
        setInstallRef("");
        setInstallName("");
        await refreshWorkspace();
        setBanner(t("componentLibraryInstalledFromUrl"));
      } catch (error) {
        setBanner(String(error));
      } finally {
        setBusy(false);
      }
    },
    [installUrl, installRef, installName, installScope, projectDir, refreshWorkspace]
  );

  const handleSyncLibrary = useCallback(
    async (library: ComponentLibrary) => {
      setBusy(true);
      try {
        await syncThirdPartyLibrary({
          projectDir,
          scope: library.scope,
          libraryId: library.id,
        });
        await refreshWorkspace();
        setBanner(t("componentLibrarySynced"));
      } catch (error) {
        setBanner(String(error));
      } finally {
        setBusy(false);
      }
    },
    [projectDir, refreshWorkspace]
  );

  const handleSyncAll = useCallback(async () => {
    setBusy(true);
    try {
      const count = await syncAllThirdPartyLibraries(projectDir ?? undefined);
      await refreshWorkspace();
      setBanner(tf("componentLibrarySyncAllDone", { count }));
    } catch (error) {
      setBanner(String(error));
    } finally {
      setBusy(false);
    }
  }, [projectDir, refreshWorkspace]);

  const hasSyncableLibraries = useMemo(
    () => libraries.some((lib) => lib.sourceUrl != null),
    [libraries]
  );

  const scopeOptions: Array<{ value: LibraryScopeFilter; label: string }> = [
    { value: "all", label: t("libraryScopeAll") },
    { value: "system", label: t("componentLibrariesSystem") },
    { value: "global", label: t("componentLibrariesGlobal") },
    { value: "project", label: t("componentLibrariesProject") },
  ];

  return (
    <div className="flex h-full min-h-0 w-full min-w-0 flex-col bg-surface text-[var(--text)]">
      <div className="panel-header-bar-tall flex flex-col border-b border-border">
        <div className="flex items-center justify-between gap-4 min-h-0">
          <div className="min-w-0">
            <h2 className="text-lg font-semibold">{t("libraryWorkspaceTitle")}</h2>
            <p className="mt-1 text-sm text-[var(--text-muted)]">{t("libraryWorkspaceDesc")}</p>
          </div>
          <div className="flex flex-wrap items-center gap-[var(--toolbar-gap)] shrink-0">
            <button
              type="button"
              onClick={() => void handleImportLibraryFolder("global")}
              disabled={busy}
              className="rounded bg-primary px-3 py-1.5 text-xs text-white hover:bg-blue-600 disabled:opacity-50"
            >
              {t("componentLibraryImportFolder")}
            </button>
            <button
              type="button"
              onClick={() => void handleImportLibraryFiles("global")}
              disabled={busy}
              className="rounded border border-border px-3 py-1.5 text-xs hover:bg-[var(--surface-hover)] disabled:opacity-50"
            >
              {t("componentLibraryImportFiles")}
            </button>
            <button
              type="button"
              onClick={() => void handleImportLibraryFolder("project")}
              disabled={busy || !projectDir}
              className="rounded border border-border px-3 py-1.5 text-xs hover:bg-[var(--surface-hover)] disabled:opacity-50"
            >
              {t("componentLibrariesProject")}
            </button>
            <button
              type="button"
              onClick={() => setInstallFromUrlOpen(true)}
              disabled={busy}
              className="rounded border border-border px-3 py-1.5 text-xs hover:bg-[var(--surface-hover)] disabled:opacity-50"
            >
              {t("componentLibraryInstallFromUrl")}
            </button>
            {hasSyncableLibraries && (
              <button
                type="button"
                onClick={() => void handleSyncAll()}
                disabled={busy}
                className="rounded border border-border px-3 py-1.5 text-xs hover:bg-[var(--surface-hover)] disabled:opacity-50"
              >
                {t("componentLibrarySyncAll")}
              </button>
            )}
          </div>
        </div>
        {installFromUrlOpen && (
          <div className="mt-2 rounded border border-border bg-[var(--bg-elevated)] p-3 space-y-2">
            <div className="text-sm font-medium">{t("componentLibraryInstallFromUrlTitle")}</div>
            <input
              value={installUrl}
              onChange={(e) => setInstallUrl(e.target.value)}
              placeholder={t("componentLibraryInstallFromUrlPlaceholder")}
              className="w-full rounded border border-border bg-[var(--surface)] px-2.5 py-1.5 text-xs outline-none focus:border-primary"
            />
            <input
              value={installRef}
              onChange={(e) => setInstallRef(e.target.value)}
              placeholder={t("componentLibraryInstallFromUrlRefPlaceholder")}
              className="w-full rounded border border-border bg-[var(--surface)] px-2.5 py-1.5 text-xs outline-none focus:border-primary"
            />
            <input
              value={installName}
              onChange={(e) => setInstallName(e.target.value)}
              placeholder={t("componentLibraryInstallFromUrlNamePlaceholder")}
              className="w-full rounded border border-border bg-[var(--surface)] px-2.5 py-1.5 text-xs outline-none focus:border-primary"
            />
            <div className="flex items-center gap-2">
              <select
                value={installScope}
                onChange={(e) => setInstallScope(e.target.value as "global" | "project")}
                className="rounded border border-border bg-[var(--surface)] px-2 py-1 text-xs"
              >
                <option value="global">{t("componentLibrariesGlobal")}</option>
                <option value="project">{t("componentLibrariesProject")}</option>
              </select>
              <button
                type="button"
                onClick={() => void handleInstallFromUrl()}
                disabled={busy}
                className="rounded bg-primary px-3 py-1.5 text-xs text-white hover:bg-blue-600 disabled:opacity-50"
              >
                {t("componentLibraryInstallFromUrl")}
              </button>
              <button
                type="button"
                onClick={() => setInstallFromUrlOpen(false)}
                disabled={busy}
                className="rounded border border-border px-3 py-1.5 text-xs hover:bg-[var(--surface-hover)] disabled:opacity-50"
              >
                {t("cancel")}
              </button>
            </div>
          </div>
        )}
        {banner && (
          <div className="mt-2 rounded border border-border bg-[var(--bg-elevated)] px-3 py-2 text-xs text-[var(--text-muted)]">
            {banner}
          </div>
        )}
      </div>

      <div className="flex min-h-0 min-w-0 flex-1 w-full">
        <LibrarySourceSidebar
          libraries={libraries}
          selectedLibraryId={selectedLibraryId}
          onSelectLibrary={setSelectedLibraryId}
          busy={busy}
          projectDir={projectDir}
          query={libraryQuery}
          onQueryChange={setLibraryQuery}
          enabledOnly={libraryEnabledOnly}
          onEnabledOnlyChange={setLibraryEnabledOnly}
          onToggleLibrary={handleToggleLibrary}
          onRemoveLibrary={handleRemoveLibrary}
          onSyncLibrary={handleSyncLibrary}
        />

        <section className="flex min-h-0 w-[340px] shrink-0 flex-col border-r border-border bg-surface-alt overflow-hidden">
          <div className="panel-header-bar-tall shrink-0 border-b border-border flex flex-col gap-2">
            <div className="flex items-center gap-2 text-sm font-medium">
              <SectionIcon letter="T" bg="bg-violet-500/85" title={t("libraryTypeList")} />
              {t("libraryTypeList")}
            </div>
            <div className="mt-2 flex flex-col gap-2">
              <input
                value={typeQuery}
                onChange={(event) => setTypeQuery(event.target.value)}
                placeholder={t("librarySearchPlaceholder")}
                className="w-full rounded border border-border bg-[var(--surface)] px-3 py-2 text-sm outline-none focus:border-primary"
              />
              <div className="flex items-center gap-2">
                <select
                  value={scopeFilter}
                  onChange={(event) => setScopeFilter(event.target.value as LibraryScopeFilter)}
                  className="flex-1 rounded border border-border bg-[var(--surface)] px-2 py-1.5 text-xs"
                >
                  {scopeOptions.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
                <label className="flex items-center gap-1 text-xs text-[var(--text-muted)]">
                  <input
                    type="checkbox"
                    checked={enabledOnly}
                    onChange={(event) => setEnabledOnly(event.target.checked)}
                  />
                  {t("libraryEnabledOnly")}
                </label>
              </div>
            </div>
            <div className="mt-2 flex items-center justify-between text-xs text-[var(--text-muted)]">
              <span>
                {typeItems.length} / {typeTotal} {t("librarySearchResults")}
              </span>
              {selectedLibrary && <span className="truncate max-w-[120px]">{selectedLibrary.displayName}</span>}
            </div>
          </div>
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <div className="min-h-0 flex-1 overflow-y-auto">
              {typeItems.length === 0 && !typesBusy ? (
                <div className="px-4 py-6 text-sm text-[var(--text-muted)]">{t("libraryNoResults")}</div>
              ) : (
                <div className="py-1">
                  {sortedChildEntries(typeTree).map(([, child]) => (
                    <TypeTreeLevel
                      key={child.fullPath}
                      node={child}
                      depth={0}
                      expandedPaths={expandedPaths}
                      toggleExpanded={toggleExpanded}
                      selectedKey={selectedKey}
                      onSelect={setSelectedKey}
                      componentKey={componentKey}
                    />
                  ))}
                  {typesBusy && (
                    <div className="px-4 py-3 text-xs text-[var(--text-muted)]">{t("loading")}</div>
                  )}
                </div>
              )}
            </div>
            {typeHasMore && (
              <div className="shrink-0 border-t border-border p-3">
                <button
                  type="button"
                  onClick={() => void loadTypePage(typeItems.length, true)}
                  disabled={typesBusy}
                  className="w-full rounded border border-border px-3 py-2 text-xs hover:bg-[var(--surface-hover)] disabled:opacity-50"
                >
                  {t("libraryLoadMore")}
                </button>
              </div>
            )}
          </div>
        </section>

        <section className="min-w-0 flex-1 min-h-0 border-r border-border bg-surface flex flex-col">
          <div className="panel-header-bar flex items-center justify-between border-b border-border">
            <div className="flex items-center gap-2 text-sm font-medium">
              <SectionIcon letter="D" bg="bg-violet-500/85" title={t("libraryDetailsTitle")} />
              {t("libraryDetailsTitle")}
            </div>
            {selectedClass && onOpenType && (
              <button
                type="button"
                className="rounded border border-border px-2 py-1 text-xs hover:bg-[var(--surface-hover)]"
                onClick={() => onOpenType(selectedClass.qualifiedName, selectedClass.libraryId)}
              >
                {t("libraryOpenReadOnly")}
              </button>
            )}
          </div>
          <div
            className="min-h-0 flex-1 overflow-auto px-4 py-4"
            onContextMenu={(event) => {
              if (!selectedClass) return;
              event.preventDefault();
              setDetailsMenuPosition({ x: event.clientX, y: event.clientY });
              setDetailsMenuVisible(true);
            }}
          >
            {!selectedClass ? (
              <div className="text-sm text-[var(--text-muted)]">{t("libraryNoSelection")}</div>
            ) : detailBusy ? (
              <div className="text-sm text-[var(--text-muted)]">{t("loading")}</div>
            ) : detail ? (
              <div className="space-y-5 max-w-full">
                <section className="rounded border border-border bg-[var(--bg-elevated)] p-4 break-words">
                  <div className="text-lg font-semibold">{detail.qualifiedName ?? detail.name}</div>
                  <div className="mt-2 flex flex-wrap gap-3 text-xs text-[var(--text-muted)]">
                    <span>{detail.libraryName}</span>
                    <span>{detail.libraryScope}</span>
                    <span>{detail.kind}</span>
                  </div>
                  {detail.summary && <div className="mt-3 text-sm text-[var(--text)]">{detail.summary}</div>}
                  {detail.path && (
                    <div className="mt-2 text-xs text-[var(--text-muted)]">
                      {t("librarySourcePath")}: {detail.path}
                    </div>
                  )}
                  <div className="mt-2 text-xs text-[var(--text-muted)]">
                    {t("libraryMetadataSource")}: {detail.metadataSource}
                  </div>
                </section>

                <section className="rounded border border-border bg-[var(--bg-elevated)] p-4 break-words">
                  <div className="text-sm text-[var(--text-muted)]">
                    {detail.description ? (
                      detail.description.trim().startsWith("<") ? (
                        <div
                          className="prose prose-sm max-w-none prose-invert"
                          dangerouslySetInnerHTML={{ __html: detail.description }}
                        />
                      ) : (
                        <div className="whitespace-pre-wrap">{detail.description}</div>
                      )
                    ) : (
                      <span>{t("libraryDetailsEmpty")}</span>
                    )}
                  </div>
                </section>

                <section className="rounded border border-border bg-[var(--bg-elevated)] p-4 break-words">
                  <div className="text-sm font-medium">{t("libraryExtends")}</div>
                  <div className="mt-2 flex flex-wrap gap-2">
                    {(detail.extendsNames ?? []).length > 0 ? (
                      (detail.extendsNames ?? []).map((name) => (
                        <span key={name} className="rounded bg-[var(--surface)] px-2 py-1 text-xs text-[var(--text-muted)]">
                          {name}
                        </span>
                      ))
                    ) : (
                      <span className="text-xs text-[var(--text-muted)]">{t("none")}</span>
                    )}
                  </div>
                </section>

                <section className="rounded border border-border bg-[var(--bg-elevated)] p-4 break-words">
                  <div className="text-sm font-medium">{t("libraryParameters")}</div>
                  <div className="mt-2 space-y-2">
                    {detail.parameters.length > 0 ? (
                      detail.parameters.map((parameter) => (
                        <div key={parameter.name} className="rounded border border-border bg-[var(--surface)] px-3 py-2">
                          <div className="flex flex-wrap items-center gap-2 text-sm">
                            <span className="font-medium">{parameter.name}</span>
                            <span className="text-[var(--text-muted)]">{parameter.typeName}</span>
                            {parameter.defaultValue && (
                              <span className="text-[var(--text-muted)]">= {parameter.defaultValue}</span>
                            )}
                          </div>
                          {parameter.description && (
                            <div className="mt-1 text-xs text-[var(--text-muted)]">{parameter.description}</div>
                          )}
                        </div>
                      ))
                    ) : (
                      <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
                    )}
                  </div>
                </section>

                <section className="rounded border border-border bg-[var(--bg-elevated)] p-4 break-words">
                  <div className="text-sm font-medium">{t("libraryConnectors")}</div>
                  <div className="mt-2 space-y-2">
                    {(detail.connectors ?? []).length > 0 ? (
                      (detail.connectors ?? []).map((connector) => (
                        <div key={connector.name} className="rounded border border-border bg-[var(--surface)] px-3 py-2">
                          <div className="flex flex-wrap items-center gap-2 text-sm">
                            <span className="font-medium">{connector.name}</span>
                            <span className="text-[var(--text-muted)]">{connector.typeName}</span>
                            <span className="rounded bg-[var(--surface-hover)] px-2 py-0.5 text-[11px] text-[var(--text-muted)]">
                              {connector.direction}
                            </span>
                          </div>
                          {connector.description && (
                            <div className="mt-1 text-xs text-[var(--text-muted)]">{connector.description}</div>
                          )}
                        </div>
                      ))
                    ) : (
                      <div className="text-xs text-[var(--text-muted)]">{t("none")}</div>
                    )}
                  </div>
                </section>

                <section className="rounded border border-border bg-[var(--bg-elevated)] p-4 break-words">
                  <div className="text-sm font-medium">{t("libraryExamples")}</div>
                  <div className="mt-2 space-y-2">
                    {(detail.examples ?? []).length > 0 ? (
                      (detail.examples ?? []).map((example) => (
                        <div key={`${example.title}-${example.modelPath ?? ""}`} className="rounded border border-border bg-[var(--surface)] px-3 py-2">
                          <div className="text-sm font-medium">{example.title}</div>
                          {example.description && (
                            <div className="mt-1 text-xs text-[var(--text-muted)]">{example.description}</div>
                          )}
                          {example.modelPath && (
                            <div className="mt-1 text-[11px] text-[var(--text-muted)]">{example.modelPath}</div>
                          )}
                          {example.usage && (
                            <div className="mt-2 whitespace-pre-wrap text-xs text-[var(--text-muted)]">{example.usage}</div>
                          )}
                        </div>
                      ))
                    ) : (
                      <div className="text-xs text-[var(--text-muted)]">{t("libraryExamplesEmpty")}</div>
                    )}
                  </div>
                </section>

                <section className="rounded border border-border bg-[var(--bg-elevated)] p-4">
                  <div className="text-sm font-medium">{t("libraryUsageHelp")}</div>
                  <div className="mt-2 whitespace-pre-wrap text-sm text-[var(--text-muted)]">
                    {detail.usageHelp || t("libraryUsageHelpEmpty")}
                  </div>
                </section>
              </div>
            ) : (
              <div className="text-sm text-[var(--text-muted)]">{t("libraryDetailsEmpty")}</div>
            )}
          </div>
        </section>

        <section className="min-w-0 flex-1 flex-[1.1] w-full bg-[var(--bg-elevated)]">
          <div className="flex h-full min-h-0 w-full flex-col">
            <div className="min-h-0 flex-[0.58] flex flex-col border-b border-border">
              <div className="panel-header-bar shrink-0 flex items-center gap-2 border-b border-border">
                <SectionIcon letter="S" bg="bg-violet-500/85" title={t("librarySourcePreview")} />
                <span className="text-sm font-medium">{t("librarySourcePreview")}</span>
              </div>
              <div className="min-h-0 flex-1">
                <Editor
                  height="100%"
                  defaultLanguage="modelica"
                  language="modelica"
                  theme={theme === "light" ? "vs-light" : "vs-dark"}
                  value={source?.content ?? ""}
                  options={{
                    readOnly: true,
                    minimap: { enabled: false },
                    scrollBeyondLastLine: false,
                    wordWrap: "on",
                    lineNumbersMinChars: 3,
                  }}
                  beforeMount={(monaco) => {
                    if (monaco.languages.getLanguages().some((lang: { id: string }) => lang.id === "modelica")) return;
                    monaco.languages.register({ id: "modelica" });
                    monaco.languages.setMonarchTokensProvider("modelica", {
                      defaultToken: "",
                      tokenPostfix: ".mo",
                      keywords: [
                        "model", "end", "equation", "algorithm", "initial", "extends",
                        "parameter", "flow", "connect", "if", "then", "else", "elseif",
                        "for", "loop", "in", "while", "when", "elsewhen", "partial",
                        "input", "output", "package", "constant", "terminal", "function",
                        "each", "redeclare", "annotation", "assert", "terminate",
                        "operator", "type", "external", "replaceable", "record", "block",
                        "class", "connector", "reinit",
                      ],
                      typeKeywords: ["Real", "Integer", "Boolean", "String"],
                      operators: ["=", ":=", "+", "-", "*", "/", "^", "and", "or", "not"],
                      tokenizer: {
                        root: [
                          [/\b(parameter|constant|flow|discrete|input|output)\b/, "keyword"],
                          [/\b(model|block|class|connector|record|package|function|operator)\b/, "keyword"],
                          [/\b(equation|algorithm|initial|extends|each|redeclare)\b/, "keyword"],
                          [/\b(if|then|else|elseif|for|loop|in|while|when|elsewhen)\b/, "keyword"],
                          [/\b(connect|reinit|assert|terminate|annotation|external)\b/, "keyword"],
                          [/\b(end|partial|replaceable|type)\b/, "keyword"],
                          [/\b(der|pre)\s*\(/, "keyword"],
                          [/\b(Real|Integer|Boolean|String)\b/, "type"],
                          [/"[^"]*"/, "string"],
                          [/\/\/.*$/, "comment"],
                          [/\/\*/, "comment", "@comment"],
                          [/\d+\.?\d*([eE][+-]?\d+)?/, "number"],
                          [/[{}()\[\];,]/, "delimiter"],
                          [/[=:]/, "operator"],
                          [/[+\-*\/^]/, "operator"],
                          [/\b(and|or|not)\b/, "operator"],
                        ],
                        comment: [
                          [/[^\/*]+/, "comment"],
                          [/\*\//, "comment", "@pop"],
                          [/[\/*]/, "comment"],
                        ],
                      },
                    });
                  }}
                />
              </div>
            </div>
            <div className="min-h-0 flex-[0.42]">
              <LibraryRelationGraphPane
                code={source?.content ?? null}
                modelName={selectedClass?.qualifiedName ?? null}
                projectDir={projectDir}
              />
            </div>
          </div>
        </section>
      </div>
      <ContextMenu
        visible={detailsMenuVisible}
        x={detailsMenuPosition.x}
        y={detailsMenuPosition.y}
        onClose={() => setDetailsMenuVisible(false)}
        items={
          selectedClass
            ? [
                {
                  id: "open-type",
                  label: t("libraryOpenReadOnly"),
                  onClick: () => {
                    onOpenType?.(selectedClass.qualifiedName, selectedClass.libraryId);
                  },
                },
              ]
            : []
        }
      />
    </div>
  );
}
