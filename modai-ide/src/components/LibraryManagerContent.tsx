import { useCallback, useEffect, useState } from "react";
import {
  addComponentLibrary,
  listComponentLibraries,
  pickComponentLibraryFiles,
  pickComponentLibraryFolder,
  removeComponentLibrary,
  setComponentLibraryEnabled,
} from "../api/tauri";
import { t } from "../i18n";
import type { ComponentLibrary } from "../types";

interface LibraryManagerContentProps {
  projectDir?: string | null;
  onLibrariesChanged?: () => void;
}

export function LibraryManagerContent({
  projectDir = null,
  onLibrariesChanged,
}: LibraryManagerContentProps) {
  const [componentLibraries, setComponentLibraries] = useState<ComponentLibrary[]>([]);
  const [libraryBanner, setLibraryBanner] = useState<string | null>(null);
  const [libraryBusy, setLibraryBusy] = useState(false);

  const loadComponentLibraries = useCallback(() => {
    listComponentLibraries(projectDir)
      .then((items) => setComponentLibraries(items))
      .catch(() => setComponentLibraries([]));
  }, [projectDir]);

  useEffect(() => {
    loadComponentLibraries();
  }, [loadComponentLibraries]);

  useEffect(() => {
    if (!libraryBanner) return;
    const tm = setTimeout(() => setLibraryBanner(null), 3000);
    return () => clearTimeout(tm);
  }, [libraryBanner]);

  const handleImportLibraryFolder = useCallback(async (scope: "global" | "project") => {
    if (scope === "project" && !projectDir) {
      setLibraryBanner(t("componentLibrariesProjectUnavailable"));
      return;
    }
    setLibraryBusy(true);
    try {
      const selectedPath = await pickComponentLibraryFolder();
      if (!selectedPath) return;
      await addComponentLibrary({
        projectDir,
        scope,
        kind: "folder",
        sourcePath: selectedPath,
      });
      await loadComponentLibraries();
      onLibrariesChanged?.();
      setLibraryBanner(t("componentLibraryImported"));
    } catch (error) {
      setLibraryBanner(String(error));
    } finally {
      setLibraryBusy(false);
    }
  }, [projectDir, loadComponentLibraries, onLibrariesChanged]);

  const handleImportLibraryFiles = useCallback(async (scope: "global" | "project") => {
    if (scope === "project" && !projectDir) {
      setLibraryBanner(t("componentLibrariesProjectUnavailable"));
      return;
    }
    setLibraryBusy(true);
    try {
      const selectedPaths = await pickComponentLibraryFiles();
      if (selectedPaths.length === 0) return;
      for (const sourcePath of selectedPaths) {
        await addComponentLibrary({
          projectDir,
          scope,
          kind: "file",
          sourcePath,
        });
      }
      await loadComponentLibraries();
      onLibrariesChanged?.();
      setLibraryBanner(t("componentLibraryImported"));
    } catch (error) {
      setLibraryBanner(String(error));
    } finally {
      setLibraryBusy(false);
    }
  }, [projectDir, loadComponentLibraries, onLibrariesChanged]);

  const handleToggleLibrary = useCallback(async (library: ComponentLibrary) => {
    if (library.builtIn) return;
    setLibraryBusy(true);
    try {
      await setComponentLibraryEnabled({
        projectDir,
        scope: library.scope,
        libraryId: library.id,
        enabled: !library.enabled,
      });
      await loadComponentLibraries();
      onLibrariesChanged?.();
    } catch (error) {
      setLibraryBanner(String(error));
    } finally {
      setLibraryBusy(false);
    }
  }, [projectDir, loadComponentLibraries, onLibrariesChanged]);

  const handleRemoveLibrary = useCallback(async (library: ComponentLibrary) => {
    if (library.builtIn) return;
    setLibraryBusy(true);
    try {
      await removeComponentLibrary({
        projectDir,
        scope: library.scope,
        libraryId: library.id,
      });
      await loadComponentLibraries();
      onLibrariesChanged?.();
      setLibraryBanner(t("componentLibraryRemoved"));
    } catch (error) {
      setLibraryBanner(String(error));
    } finally {
      setLibraryBusy(false);
    }
  }, [projectDir, loadComponentLibraries, onLibrariesChanged]);

  const renderLibraryList = (scope: "system" | "global" | "project") => {
    const items = componentLibraries.filter((library) => library.scope === scope);
    if (items.length === 0) {
      return <div className="text-xs text-[var(--text-muted)]">{t("componentLibrariesEmpty")}</div>;
    }
    return (
      <div className="space-y-2">
        {items.map((library) => (
          <div key={library.id} className="rounded border border-border bg-[var(--surface)] px-3 py-2">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0 flex-1">
                <div className="truncate text-sm text-[var(--text)]">{library.displayName}</div>
                <div className="truncate text-[11px] text-[var(--text-muted)]">{library.kind}</div>
                {library.sourcePath && (
                  <div className="truncate text-[11px] text-[var(--text-muted)]">{library.sourcePath}</div>
                )}
              </div>
              {!library.builtIn && (
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => handleToggleLibrary(library)}
                    disabled={libraryBusy}
                    className="px-2 py-1 text-[11px] rounded border border-border hover:bg-[var(--surface-hover)] disabled:opacity-50"
                  >
                    {library.enabled ? t("disable") : t("enable")}
                  </button>
                  <button
                    type="button"
                    onClick={() => handleRemoveLibrary(library)}
                    disabled={libraryBusy}
                    className="px-2 py-1 text-[11px] rounded border border-border hover:bg-[var(--surface-hover)] disabled:opacity-50"
                  >
                    {t("deleteTest")}
                  </button>
                </div>
              )}
            </div>
          </div>
        ))}
      </div>
    );
  };

  return (
    <div className="max-w-4xl mx-auto p-6 text-[var(--text)]">
      <h2 className="text-lg font-semibold text-[var(--text)] mb-2">{t("componentLibraryManager")}</h2>
      <p className="text-sm text-[var(--text-muted)] mb-6">{t("componentLibraryManagerDesc")}</p>
      {libraryBanner && (
        <div className="mb-4 px-3 py-2 rounded text-xs bg-green-900/30 text-green-300 border border-green-700">
          {libraryBanner}
        </div>
      )}
      <div className="space-y-6">
        <section className="rounded border border-border bg-[var(--bg-elevated)] p-4">
          <h3 className="text-sm font-medium mb-1">{t("componentLibrariesSystem")}</h3>
          <p className="text-xs text-[var(--text-muted)] mb-3">{t("componentLibrariesSystemDesc")}</p>
          {renderLibraryList("system")}
        </section>
        <section className="rounded border border-border bg-[var(--bg-elevated)] p-4">
          <div className="flex items-start justify-between gap-4 mb-3">
            <div>
              <h3 className="text-sm font-medium mb-1">{t("componentLibrariesGlobal")}</h3>
              <p className="text-xs text-[var(--text-muted)]">{t("componentLibrariesGlobalDesc")}</p>
            </div>
            <div className="flex gap-2 flex-wrap">
              <button
                type="button"
                onClick={() => handleImportLibraryFolder("global")}
                disabled={libraryBusy}
                className="px-3 py-1.5 text-xs rounded bg-primary hover:bg-blue-600 text-white disabled:opacity-40"
              >
                {t("componentLibraryImportFolder")}
              </button>
              <button
                type="button"
                onClick={() => handleImportLibraryFiles("global")}
                disabled={libraryBusy}
                className="px-3 py-1.5 text-xs rounded border border-border hover:bg-[var(--surface-hover)] disabled:opacity-40"
              >
                {t("componentLibraryImportFiles")}
              </button>
            </div>
          </div>
          {renderLibraryList("global")}
        </section>
        <section className="rounded border border-border bg-[var(--bg-elevated)] p-4">
          <div className="flex items-start justify-between gap-4 mb-3">
            <div>
              <h3 className="text-sm font-medium mb-1">{t("componentLibrariesProject")}</h3>
              <p className="text-xs text-[var(--text-muted)]">{t("componentLibrariesProjectDesc")}</p>
            </div>
            <div className="flex gap-2 flex-wrap">
              <button
                type="button"
                onClick={() => handleImportLibraryFolder("project")}
                disabled={libraryBusy || !projectDir}
                className="px-3 py-1.5 text-xs rounded bg-primary hover:bg-blue-600 text-white disabled:opacity-40"
              >
                {t("componentLibraryImportFolder")}
              </button>
              <button
                type="button"
                onClick={() => handleImportLibraryFiles("project")}
                disabled={libraryBusy || !projectDir}
                className="px-3 py-1.5 text-xs rounded border border-border hover:bg-[var(--surface-hover)] disabled:opacity-40"
              >
                {t("componentLibraryImportFiles")}
              </button>
            </div>
          </div>
          {!projectDir && (
            <div className="text-xs text-[var(--text-muted)] mb-3">{t("componentLibrariesProjectUnavailable")}</div>
          )}
          {renderLibraryList("project")}
        </section>
      </div>
    </div>
  );
}
