import { useCallback, useEffect, useMemo, useState } from "react";
import Editor from "@monaco-editor/react";
import {
  addComponentLibrary,
  getComponentTypeDetails,
  listComponentLibraries,
  pickComponentLibraryFiles,
  pickComponentLibraryFolder,
  queryComponentLibraryTypes,
  readComponentTypeSource,
  removeComponentLibrary,
  setComponentLibraryEnabled,
} from "../api/tauri";
import { t } from "../i18n";
import type {
  ComponentLibrary,
  ComponentTypeInfo,
  ComponentTypeSource,
  InstantiableClass,
} from "../types";
import { LibraryRelationGraphPane } from "./LibraryRelationGraphPane";
import { LibrarySourceSidebar } from "./LibrarySourceSidebar";

interface ComponentLibraryWorkspaceProps {
  projectDir?: string | null;
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
  theme,
  onLibrariesChanged,
  onOpenType,
}: ComponentLibraryWorkspaceProps) {
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
    try {
      const items = await listComponentLibraries(projectDir);
      setLibraries(items);
    } catch (error) {
      setLibraries([]);
      setBanner(String(error));
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
      }
    },
    [debouncedTypeQuery, enabledOnly, projectDir, scopeFilter, selectedLibraryId]
  );

  useEffect(() => {
    void loadLibraries();
  }, [loadLibraries]);

  useEffect(() => {
    setSelectedKey(null);
    setDetail(null);
    setSource(null);
    void loadTypePage(0, false);
  }, [loadTypePage]);

  const refreshWorkspace = useCallback(async () => {
    await loadLibraries();
    await loadTypePage(0, false);
    onLibrariesChanged?.();
  }, [loadLibraries, loadTypePage, onLibrariesChanged]);

  useEffect(() => {
    if (typeItems.length === 0) {
      setSelectedKey(null);
      return;
    }
    if (!selectedKey || !typeItems.some((item) => componentKey(item) === selectedKey)) {
      setSelectedKey(componentKey(typeItems[0]));
    }
  }, [selectedKey, typeItems]);

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
          </div>
        </div>
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
        />

        <section className="w-[340px] shrink-0 border-r border-border bg-surface-alt">
          <div className="panel-header-bar-tall border-b border-border flex flex-col gap-2">
            <div className="text-sm font-medium">{t("libraryTypeList")}</div>
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
          <div className="flex h-full flex-col overflow-hidden">
            <div className="flex-1 overflow-auto">
              {typeItems.length === 0 && !typesBusy ? (
                <div className="px-4 py-6 text-sm text-[var(--text-muted)]">{t("libraryNoResults")}</div>
              ) : (
                <div className="divide-y divide-border">
                  {typeItems.map((item) => {
                    const active = componentKey(item) === selectedKey;
                    return (
                      <button
                        key={componentKey(item)}
                        type="button"
                        className={`w-full px-4 py-3 text-left transition-colors ${
                          active ? "bg-[var(--surface-active)]" : "hover:bg-[var(--surface-hover)]"
                        }`}
                        onClick={() => setSelectedKey(componentKey(item))}
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
                          <span className="shrink-0 rounded bg-[var(--surface)] px-2 py-0.5 text-[10px] text-[var(--text-muted)]">
                            {item.libraryScope}
                          </span>
                        </div>
                      </button>
                    );
                  })}
                  {typesBusy && (
                    <div className="px-4 py-3 text-xs text-[var(--text-muted)]">{t("loading")}</div>
                  )}
                </div>
              )}
            </div>
            {typeHasMore && (
              <div className="border-t border-border p-3">
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

        <section className="min-w-0 flex-1 border-r border-border bg-surface">
          <div className="panel-header-bar flex items-center justify-between border-b border-border">
            <div className="text-sm font-medium">{t("libraryDetailsTitle")}</div>
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
          <div className="h-full overflow-auto px-4 py-4">
            {!selectedClass ? (
              <div className="text-sm text-[var(--text-muted)]">{t("libraryNoSelection")}</div>
            ) : detailBusy ? (
              <div className="text-sm text-[var(--text-muted)]">{t("loading")}</div>
            ) : detail ? (
              <div className="space-y-5">
                <section className="rounded border border-border bg-[var(--bg-elevated)] p-4">
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

                <section className="rounded border border-border bg-[var(--bg-elevated)] p-4">
                  <div className="text-sm font-medium">{t("libraryDescription")}</div>
                  <div className="mt-2 whitespace-pre-wrap text-sm text-[var(--text-muted)]">
                    {detail.description || t("libraryDetailsEmpty")}
                  </div>
                </section>

                <section className="rounded border border-border bg-[var(--bg-elevated)] p-4">
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

                <section className="rounded border border-border bg-[var(--bg-elevated)] p-4">
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

                <section className="rounded border border-border bg-[var(--bg-elevated)] p-4">
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

                <section className="rounded border border-border bg-[var(--bg-elevated)] p-4">
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
              <div className="panel-header-bar shrink-0 flex items-center border-b border-border">
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
    </div>
  );
}
