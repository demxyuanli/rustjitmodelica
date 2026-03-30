import { t } from "../../i18n";

export interface ComponentLibraryWorkspaceChromeProps {
  busy: boolean;
  projectDir: string | null;
  hasSyncableLibraries: boolean;
  onImportFolderGlobal: () => void;
  onImportFilesGlobal: () => void;
  onImportFolderProject: () => void;
  onOpenInstallUrl: () => void;
  onSyncAll: () => void;
  installFromUrlOpen: boolean;
  installUrl: string;
  onInstallUrlChange: (value: string) => void;
  installRef: string;
  onInstallRefChange: (value: string) => void;
  installName: string;
  onInstallNameChange: (value: string) => void;
  installScope: "global" | "project";
  onInstallScopeChange: (value: "global" | "project") => void;
  onInstallSubmit: () => void;
  onInstallCancel: () => void;
  banner: string | null;
}

export function ComponentLibraryWorkspaceChrome({
  busy,
  projectDir,
  hasSyncableLibraries,
  onImportFolderGlobal,
  onImportFilesGlobal,
  onImportFolderProject,
  onOpenInstallUrl,
  onSyncAll,
  installFromUrlOpen,
  installUrl,
  onInstallUrlChange,
  installRef,
  onInstallRefChange,
  installName,
  onInstallNameChange,
  installScope,
  onInstallScopeChange,
  onInstallSubmit,
  onInstallCancel,
  banner,
}: ComponentLibraryWorkspaceChromeProps) {
  return (
    <div className="panel-header-bar-tall flex flex-col border-b border-border">
      <div className="flex items-center justify-between gap-4 min-h-0">
        <div className="min-w-0">
          <h2 className="text-lg font-semibold">{t("libraryWorkspaceTitle")}</h2>
          <p className="mt-1 text-sm text-[var(--text-muted)]">{t("libraryWorkspaceDesc")}</p>
        </div>
        <div className="flex flex-wrap items-center gap-[var(--toolbar-gap)] shrink-0">
          <button
            type="button"
            onClick={onImportFolderGlobal}
            disabled={busy}
            className="rounded bg-primary px-3 py-1.5 text-xs text-white hover:bg-blue-600 disabled:opacity-50"
          >
            {t("componentLibraryImportFolder")}
          </button>
          <button
            type="button"
            onClick={onImportFilesGlobal}
            disabled={busy}
            className="rounded border border-border px-3 py-1.5 text-xs hover:bg-[var(--surface-hover)] disabled:opacity-50"
          >
            {t("componentLibraryImportFiles")}
          </button>
          <button
            type="button"
            onClick={onImportFolderProject}
            disabled={busy || !projectDir}
            className="rounded border border-border px-3 py-1.5 text-xs hover:bg-[var(--surface-hover)] disabled:opacity-50"
          >
            {t("componentLibrariesProject")}
          </button>
          <button
            type="button"
            onClick={onOpenInstallUrl}
            disabled={busy}
            className="rounded border border-border px-3 py-1.5 text-xs hover:bg-[var(--surface-hover)] disabled:opacity-50"
          >
            {t("componentLibraryInstallFromUrl")}
          </button>
          {hasSyncableLibraries && (
            <button
              type="button"
              onClick={onSyncAll}
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
            onChange={(e) => onInstallUrlChange(e.target.value)}
            placeholder={t("componentLibraryInstallFromUrlPlaceholder")}
            className="w-full rounded border border-border bg-[var(--surface)] px-2.5 py-1.5 text-xs outline-none focus:border-primary"
          />
          <input
            value={installRef}
            onChange={(e) => onInstallRefChange(e.target.value)}
            placeholder={t("componentLibraryInstallFromUrlRefPlaceholder")}
            className="w-full rounded border border-border bg-[var(--surface)] px-2.5 py-1.5 text-xs outline-none focus:border-primary"
          />
          <input
            value={installName}
            onChange={(e) => onInstallNameChange(e.target.value)}
            placeholder={t("componentLibraryInstallFromUrlNamePlaceholder")}
            className="w-full rounded border border-border bg-[var(--surface)] px-2.5 py-1.5 text-xs outline-none focus:border-primary"
          />
          <div className="flex items-center gap-2">
            <select
              value={installScope}
              onChange={(e) => onInstallScopeChange(e.target.value as "global" | "project")}
              className="rounded border border-border bg-[var(--surface)] px-2 py-1 text-xs"
            >
              <option value="global">{t("componentLibrariesGlobal")}</option>
              <option value="project">{t("componentLibrariesProject")}</option>
            </select>
            <button
              type="button"
              onClick={onInstallSubmit}
              disabled={busy}
              className="rounded bg-primary px-3 py-1.5 text-xs text-white hover:bg-blue-600 disabled:opacity-50"
            >
              {t("componentLibraryInstallFromUrl")}
            </button>
            <button
              type="button"
              onClick={onInstallCancel}
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
  );
}
