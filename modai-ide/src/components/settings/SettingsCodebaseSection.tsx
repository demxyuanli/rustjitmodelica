import { FolderOpen, FileText, RefreshCw, Trash2, X } from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { indexListIncludedFiles } from "../../api/tauri";
import { t } from "../../i18n";
import type { AppSettingsForm, IndexActionState } from "./settingsTypes";
import { SettingsRow, SettingsSwitch } from "./settingsPrimitives";
import { SettingsRebuildLibraryButton } from "./SettingsRebuildLibraryButton";

export interface SettingsCodebaseSectionProps {
  indexFileCount: number;
  indexSymbolCount: number;
  indexState?: "idle" | "building" | "ready" | null;
  indexAction?: IndexActionState;
  onIndexRefresh?: () => void;
  onIndexRebuild?: () => void;
  indexPct: number;
  projectDir?: string | null;
  appSettings?: AppSettingsForm;
  onAppSettingsChange?: (s: AppSettingsForm) => void;
  includedFilesResult: { total: number; paths: string[] } | null;
  setIncludedFilesResult: (v: { total: number; paths: string[] } | null) => void;
}

export function SettingsCodebaseSection({
  indexFileCount,
  indexSymbolCount,
  indexState,
  indexAction,
  onIndexRefresh,
  onIndexRebuild,
  indexPct,
  projectDir,
  appSettings,
  onAppSettingsChange,
  includedFilesResult,
  setIncludedFilesResult,
}: SettingsCodebaseSectionProps) {
  return (
    <section id="settings-group-codebase">
      <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionIndexingDocs")}</h3>
      <div className="space-y-6">
        <div>
          <h4 className="text-xs font-semibold text-[var(--text-muted)] uppercase tracking-wider mb-2">{t("settingsSubsectionIndexOverview")}</h4>
          <div className="space-y-0">
            <div className="flex items-start justify-between gap-4 py-4 border-b border-[var(--border)]">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-1.5">
                  <span className="text-sm font-medium text-[var(--text)]">{t("settingsIndex")}</span>
                  <span className="text-[var(--text-muted)] opacity-70" title={t("settingsIndexDesc")} aria-hidden="true">&#9432;</span>
                </div>
                <p className="text-xs text-[var(--text-muted)] mt-1">{t("settingsIndexDesc")}</p>
                {indexAction?.running && (
                  <div className="mt-2 space-y-1">
                    <div className="flex items-center justify-between text-[11px] text-[var(--text-muted)]">
                      <span>{indexAction.action === "rebuild" ? t("indexRebuilding") : t("indexRefreshing")}</span>
                      <span>{indexAction.done} / {indexAction.total}</span>
                    </div>
                    <div className="w-full h-1.5 bg-[var(--surface)] rounded-full overflow-hidden max-w-[200px]">
                      <div className="h-full bg-primary rounded-full transition-all duration-200"
                        style={{ width: indexAction.total > 0 ? `${Math.round((indexAction.done / indexAction.total) * 100)}%` : "0%" }} />
                    </div>
                  </div>
                )}
                {!indexAction?.running && (
                  <div className="mt-2 flex items-center gap-2">
                    <div className="w-full h-1.5 bg-[var(--surface)] rounded-full overflow-hidden max-w-[120px]">
                      <div className="h-full bg-green-600 rounded-full transition-all duration-200" style={{ width: `${indexPct}%` }} />
                    </div>
                    <span className="text-xs text-[var(--text-muted)]">
                      {indexState === "ready"
                        ? `${indexPct}% \u2022 ${indexFileCount} ${t("indexFiles")} \u2022 ${indexSymbolCount} ${t("indexSymbols")}`
                        : indexState === "building"
                          ? t("indexBuilding")
                          : t("indexIdle")}
                    </span>
                  </div>
                )}
              </div>
              <div className="flex-shrink-0 flex items-center gap-2">
                <button type="button" disabled={indexAction?.running ?? false} onClick={onIndexRefresh}
                  className="p-2 rounded-md bg-[var(--surface)] border border-border hover:bg-white/10 disabled:opacity-40 text-[var(--text-muted)]"
                  title={t("indexRefreshDesc")}
                  aria-label={t("indexSync")}>
                  <RefreshCw size={16} className={indexAction?.running ? "animate-spin" : undefined} />
                </button>
                <button type="button" disabled={indexAction?.running ?? false} onClick={onIndexRebuild}
                  className="p-2 rounded-md border border-border hover:bg-white/10 text-[var(--text-muted)] disabled:opacity-40"
                  title={t("indexRebuildDesc")}
                  aria-label={t("indexDelete")}>
                  <Trash2 size={16} />
                </button>
              </div>
            </div>
            <p className="text-xs text-[var(--text-muted)] mt-1 mb-2">{t("settingsAllDataStoredLocally")}</p>
          </div>
        </div>
        {onAppSettingsChange && appSettings && (
          <>
            <div>
              <h4 className="text-xs font-semibold text-[var(--text-muted)] uppercase tracking-wider mb-2">{t("settingsSubsectionIndexingBehavior")}</h4>
              <SettingsRow title={t("settingsIndexNewFolders")} description={t("settingsIndexNewFoldersDesc")}>
                <div className="flex items-center gap-2">
                  <SettingsSwitch checked={(appSettings.indexing?.indexAutoNewFolders ?? true) !== false} onChange={(v) => onAppSettingsChange({ ...appSettings, indexing: { ...appSettings.indexing, indexAutoNewFolders: v } })} ariaLabel={t("settingsIndexNewFolders")} />
                  <span className="text-xs text-[var(--text-muted)]">&lt; {appSettings.indexing?.indexAutoNewFoldersMaxFiles ?? 50000} {t("indexFiles")}</span>
                </div>
              </SettingsRow>
            </div>
            <div>
              <h4 className="text-xs font-semibold text-[var(--text-muted)] uppercase tracking-wider mb-2">{t("settingsSubsectionIgnoreRules")}</h4>
              <SettingsRow title={t("settingsIgnoreFiles")} description={t("settingsIgnoreFilesDesc")}>
                <div className="flex items-center gap-2">
                  <button type="button" onClick={() => projectDir && openPath(projectDir)} disabled={!projectDir} className="p-2 rounded-md bg-[var(--surface)] border border-border hover:bg-white/10 disabled:opacity-40 text-[var(--text-muted)]" title={t("openFolder")} aria-label={t("openFolder")}>
                    <FolderOpen size={16} />
                  </button>
                  <button
                    type="button"
                    disabled={!projectDir}
                    className="p-2 rounded-md bg-[var(--surface)] border border-border hover:bg-white/10 disabled:opacity-40 text-[var(--text-muted)]"
                    title={t("settingsViewIncludedFiles")}
                    aria-label={t("settingsViewIncludedFiles")}
                    onClick={async () => {
                      if (!projectDir) return;
                      try {
                        const r = await indexListIncludedFiles(projectDir, 500);
                        setIncludedFilesResult({ total: r.total, paths: r.paths });
                      } catch {
                        setIncludedFilesResult({ total: 0, paths: [] });
                      }
                    }}
                  >
                    <FileText size={16} />
                  </button>
                </div>
              </SettingsRow>
            </div>
            <div>
              <h4 className="text-xs font-semibold text-[var(--text-muted)] uppercase tracking-wider mb-2">{t("settingsSubsectionRepoAndComponent")}</h4>
              <SettingsRow title={t("settingsIndexRepoForGrep")} description={t("settingsIndexRepoForGrepDesc")}>
                <SettingsSwitch checked={(appSettings.indexing?.indexRepoForGrep ?? true) !== false} onChange={(v) => onAppSettingsChange({ ...appSettings, indexing: { ...appSettings.indexing, indexRepoForGrep: v } })} ariaLabel={t("settingsIndexRepoForGrep")} />
              </SettingsRow>
              <SettingsRow title={t("settingsComponentLibraryIndex")} description={t("settingsComponentLibraryIndexDesc")}>
                <SettingsSwitch checked={(appSettings.indexCache?.componentLibraryIndexEnabled ?? true) !== false} onChange={(v) => onAppSettingsChange({ ...appSettings, indexCache: { ...appSettings.indexCache, componentLibraryIndexEnabled: v } })} ariaLabel={t("settingsComponentLibraryIndex")} />
              </SettingsRow>
              <SettingsRow title={t("settingsRepoIndexRefreshOnJitLoad")} description={t("settingsRepoIndexRefreshOnJitLoadDesc")}>
                <SettingsSwitch checked={(appSettings.indexCache?.repoIndexRefreshOnJitLoad ?? true) !== false} onChange={(v) => onAppSettingsChange({ ...appSettings, indexCache: { ...appSettings.indexCache, repoIndexRefreshOnJitLoad: v } })} ariaLabel={t("settingsRepoIndexRefreshOnJitLoad")} />
              </SettingsRow>
              <SettingsRow title={t("settingsGitStatusThrottleMs")} description={t("settingsGitStatusThrottleMsDesc")}>
                <input type="number" min={500} max={60000} step={500} value={appSettings.indexCache?.gitStatusThrottleMs ?? 2000} onChange={(e) => { const v = parseInt(e.target.value, 10); if (!Number.isNaN(v)) onAppSettingsChange({ ...appSettings, indexCache: { ...appSettings.indexCache, gitStatusThrottleMs: Math.max(500, Math.min(60000, v)) } }); }} className="w-24 bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded font-mono" />
                <span className="text-xs text-[var(--text-muted)]"> ms</span>
              </SettingsRow>
              <SettingsRow title={t("indexRebuildComponentLibrary")} description={t("indexRebuildComponentLibraryDesc")}>
                <SettingsRebuildLibraryButton />
              </SettingsRow>
            </div>
          </>
        )}
      </div>
      {includedFilesResult !== null && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50" role="dialog" aria-modal="true" onClick={() => setIncludedFilesResult(null)}>
          <div className="bg-[var(--surface)] border border-border rounded-lg shadow-xl max-w-2xl w-full max-h-[80vh] flex flex-col m-4" onClick={(e) => e.stopPropagation()}>
            <div className="flex items-center justify-between p-3 border-b border-border">
              <span className="text-sm font-medium">{t("settingsViewIncludedFiles")}</span>
              <button type="button" onClick={() => setIncludedFilesResult(null)} className="p-1.5 rounded hover:bg-white/10 text-[var(--text-muted)]" aria-label={t("cancel")}><X size={16} /></button>
            </div>
            <div className="p-3 text-xs text-[var(--text-muted)]">
              {includedFilesResult.total} {t("indexFiles")}{includedFilesResult.paths.length < includedFilesResult.total ? ` (first ${includedFilesResult.paths.length})` : ""}
            </div>
            <div className="flex-1 min-h-0 overflow-auto p-3 pt-0 font-mono text-xs text-[var(--text)] break-all">
              {includedFilesResult.paths.length === 0 ? (
                <span className="text-[var(--text-muted)]">—</span>
              ) : (
                <ul className="list-none space-y-0.5">
                  {includedFilesResult.paths.map((p, i) => (
                    <li key={i}>{p}</li>
                  ))}
                </ul>
              )}
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
