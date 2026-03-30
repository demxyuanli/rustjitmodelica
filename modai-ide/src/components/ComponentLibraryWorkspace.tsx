import { useCallback, useEffect, useMemo, useState } from "react";
import type { AppSettings } from "../api/tauri";
import { dependencyGraphBehaviorFromAppSettings } from "../utils/dependencyGraphBehavior";
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
import type { ComponentLibrary, ComponentTypeInfo, ComponentTypeSource, InstantiableClass } from "../types";
import { buildTypeTree, sortedChildEntries } from "./componentLibrary/typeTree";
import { componentKey } from "./componentLibrary/componentLibraryKeys";
import { ComponentLibrarySourcePreviewColumn } from "./componentLibrary/ComponentLibrarySourcePreviewColumn";
import { ComponentLibraryTypeDetailPane } from "./componentLibrary/ComponentLibraryTypeDetailPane";
import {
  ComponentLibraryTypeTreePane,
  type LibraryScopeFilter,
} from "./componentLibrary/ComponentLibraryTypeTreePane";
import { ComponentLibraryWorkspaceChrome } from "./componentLibrary/ComponentLibraryWorkspaceChrome";
import { LibrarySourceSidebar } from "./LibrarySourceSidebar";
import { ContextMenu } from "./ContextMenu";

interface ComponentLibraryWorkspaceProps {
  projectDir?: string | null;
  /** When false, defer list/query until true to avoid startup cost when workspace is hidden */
  isActive?: boolean;
  theme: "dark" | "light";
  onLibrariesChanged?: () => void;
  onOpenType?: (typeName: string, libraryId?: string) => void;
  onOpenDependencyGraphSettings?: () => void;
  appSettings?: AppSettings | null;
}

const PAGE_SIZE = 120;

export function ComponentLibraryWorkspace({
  projectDir = null,
  isActive = true,
  theme,
  onLibrariesChanged,
  onOpenType,
  onOpenDependencyGraphSettings,
  appSettings = null,
}: ComponentLibraryWorkspaceProps) {
  const dependencyGraphBehavior = useMemo(
    () => dependencyGraphBehaviorFromAppSettings(appSettings),
    [appSettings]
  );
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
      <ComponentLibraryWorkspaceChrome
        busy={busy}
        projectDir={projectDir}
        hasSyncableLibraries={hasSyncableLibraries}
        onImportFolderGlobal={() => void handleImportLibraryFolder("global")}
        onImportFilesGlobal={() => void handleImportLibraryFiles("global")}
        onImportFolderProject={() => void handleImportLibraryFolder("project")}
        onOpenInstallUrl={() => setInstallFromUrlOpen(true)}
        onSyncAll={() => void handleSyncAll()}
        installFromUrlOpen={installFromUrlOpen}
        installUrl={installUrl}
        onInstallUrlChange={setInstallUrl}
        installRef={installRef}
        onInstallRefChange={setInstallRef}
        installName={installName}
        onInstallNameChange={setInstallName}
        installScope={installScope}
        onInstallScopeChange={setInstallScope}
        onInstallSubmit={() => void handleInstallFromUrl()}
        onInstallCancel={() => setInstallFromUrlOpen(false)}
        banner={banner}
      />

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

        <ComponentLibraryTypeTreePane
          typeTree={typeTree}
          expandedPaths={expandedPaths}
          toggleExpanded={toggleExpanded}
          selectedKey={selectedKey}
          onSelectKey={setSelectedKey}
          typeQuery={typeQuery}
          onTypeQueryChange={setTypeQuery}
          scopeFilter={scopeFilter}
          onScopeFilterChange={setScopeFilter}
          scopeOptions={scopeOptions}
          enabledOnly={enabledOnly}
          onEnabledOnlyChange={setEnabledOnly}
          typeItemsLength={typeItems.length}
          typeTotal={typeTotal}
          typesBusy={typesBusy}
          typeHasMore={typeHasMore}
          onLoadMore={() => void loadTypePage(typeItems.length, true)}
          selectedLibraryDisplayName={selectedLibrary?.displayName ?? null}
        />

        <ComponentLibraryTypeDetailPane
          selectedClass={selectedClass}
          detail={detail}
          detailBusy={detailBusy}
          onOpenType={onOpenType}
          onContentContextMenu={(event) => {
            if (!selectedClass) return;
            event.preventDefault();
            setDetailsMenuPosition({ x: event.clientX, y: event.clientY });
            setDetailsMenuVisible(true);
          }}
        />

        <ComponentLibrarySourcePreviewColumn
          theme={theme}
          projectDir={projectDir}
          source={source}
          selectedClass={selectedClass}
          onOpenDependencyGraphSettings={onOpenDependencyGraphSettings}
          dependencyGraphBehavior={dependencyGraphBehavior}
        />

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
