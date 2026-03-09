import { t } from "../i18n";

export interface IndexActionState {
  running: boolean;
  action: "refresh" | "rebuild" | null;
  done: number;
  total: number;
}

interface ModalsProps {
  showJitFailModal: boolean;
  jitFailErrors: string[];
  onJitFailClose: () => void;
  onJitFailYes: () => void;
  onJitFailTrySelfIterate?: () => void;
  showSettings: boolean;
  onSettingsClose: () => void;
  theme: "dark" | "light";
  onThemeChange: (theme: "dark" | "light") => void;
  onEnterDevMode?: () => void;
  indexFileCount?: number;
  indexSymbolCount?: number;
  indexState?: "idle" | "building" | "ready" | null;
  indexAction?: IndexActionState;
  onIndexRefresh?: () => void;
  onIndexRebuild?: () => void;
}

export function Modals({
  showJitFailModal,
  jitFailErrors,
  onJitFailClose,
  onJitFailYes,
  onJitFailTrySelfIterate,
  showSettings,
  onSettingsClose,
  theme,
  onThemeChange,
  onEnterDevMode,
  indexFileCount = 0,
  indexSymbolCount = 0,
  indexState,
  indexAction,
  onIndexRefresh,
  onIndexRebuild,
}: ModalsProps) {
  return (
    <>
      {showJitFailModal && (
        <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center" onClick={onJitFailClose} role="dialog" aria-modal="true" aria-labelledby="jit-fail-title">
          <div className="bg-surface-alt border border-border rounded-lg p-4 min-w-[320px] shadow-xl" onClick={(e) => e.stopPropagation()}>
            <h2 id="jit-fail-title" className="text-sm font-medium text-[var(--text)] mb-2">{t("useAiToComplete")}</h2>
            <p className="text-xs text-[var(--text-muted)] mb-3">
              {jitFailErrors.slice(0, 3).join(" ")}
              {jitFailErrors.length > 3 ? "..." : ""}
            </p>
            <div className="flex gap-2 justify-end flex-wrap">
              <button type="button" className="px-3 py-1.5 bg-gray-600 hover:bg-gray-500 text-sm rounded" onClick={onJitFailClose}>{t("no")}</button>
              <button type="button" className="px-3 py-1.5 bg-primary hover:bg-blue-600 text-sm rounded text-white" onClick={onJitFailYes}>
                {t("yes")}
              </button>
              {onJitFailTrySelfIterate && (
                <button type="button" className="px-3 py-1.5 bg-amber-600 hover:bg-amber-500 text-sm rounded text-white" onClick={onJitFailTrySelfIterate}>
                  {t("trySelfIterate")}
                </button>
              )}
            </div>
          </div>
        </div>
      )}
      {showSettings && (
        <div className="fixed inset-0 bg-black/50 z-40 flex items-center justify-center" onClick={onSettingsClose}>
          <div className="bg-surface-alt border border-border rounded-lg p-5 min-w-[400px] max-w-[480px] shadow-xl" onClick={(e) => e.stopPropagation()}>
            <div className="flex justify-between items-center mb-4">
              <span className="text-sm font-semibold text-[var(--text)]">{t("settings")}</span>
              <button type="button" className="text-[var(--text-muted)] hover:text-[var(--text)] text-xs px-2 py-1 rounded hover:bg-white/10" onClick={onSettingsClose}>{t("closeTab")}</button>
            </div>

            <div className="space-y-4">
              <section>
                <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-2">{t("settingsAppearance")}</h3>
                <div className="flex items-center gap-2">
                  <span className="text-xs text-[var(--text)]">{t("theme")}:</span>
                  <button type="button" className={`px-2.5 py-1 text-xs rounded ${theme === "dark" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] border border-border"}`} onClick={() => onThemeChange("dark")}>{t("themeDark")}</button>
                  <button type="button" className={`px-2.5 py-1 text-xs rounded ${theme === "light" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] border border-border"}`} onClick={() => onThemeChange("light")}>{t("themeLight")}</button>
                </div>
              </section>

              <section>
                <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-2">{t("settingsKeyboard")}</h3>
                <div className="text-xs text-[var(--text)] space-y-1">
                  <div className="flex justify-between"><span>Ctrl+S</span><span className="text-[var(--text-muted)]">{t("save")}</span></div>
                  <div className="flex justify-between"><span>Ctrl+B</span><span className="text-[var(--text-muted)]">{t("toggleSidebar")}</span></div>
                  <div className="flex justify-between"><span>Ctrl+J</span><span className="text-[var(--text-muted)]">{t("toggleBottomPanel")}</span></div>
                </div>
              </section>

              <section>
                <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-2">{t("settingsIndex")}</h3>
                <div className="space-y-2">
                  <div className="flex items-center gap-3 text-xs text-[var(--text)]">
                    <span>
                      {indexState === "ready"
                        ? `${indexFileCount} ${t("indexFiles")}, ${indexSymbolCount} ${t("indexSymbols")}`
                        : indexState === "building"
                        ? t("indexBuilding")
                        : t("indexIdle")}
                    </span>
                    <span className={`inline-block w-2 h-2 rounded-full ${indexState === "ready" ? "bg-green-500" : indexState === "building" ? "bg-amber-400 animate-pulse" : "bg-gray-500"}`} />
                  </div>

                  {indexAction?.running && (
                    <div className="space-y-1">
                      <div className="flex items-center justify-between text-[11px] text-[var(--text-muted)]">
                        <span>
                          {indexAction.action === "rebuild" ? t("indexRebuilding") : t("indexRefreshing")}
                        </span>
                        <span>{indexAction.done} / {indexAction.total}</span>
                      </div>
                      <div className="w-full h-1.5 bg-[var(--surface)] rounded-full overflow-hidden">
                        <div
                          className="h-full bg-primary rounded-full transition-all duration-200"
                          style={{ width: indexAction.total > 0 ? `${Math.round((indexAction.done / indexAction.total) * 100)}%` : "0%" }}
                        />
                      </div>
                    </div>
                  )}

                  <div className="flex gap-2">
                    <button
                      type="button"
                      disabled={indexAction?.running || false}
                      onClick={onIndexRefresh}
                      className="px-3 py-1.5 text-xs rounded bg-[var(--surface)] border border-border hover:bg-white/10 disabled:opacity-40"
                      title={t("indexRefreshDesc")}
                    >
                      {indexAction?.running && indexAction.action === "refresh" ? t("indexRefreshing") : t("indexRefresh")}
                    </button>
                    <button
                      type="button"
                      disabled={indexAction?.running || false}
                      onClick={onIndexRebuild}
                      className="px-3 py-1.5 text-xs rounded bg-amber-700 hover:bg-amber-600 text-white disabled:opacity-40"
                      title={t("indexRebuildDesc")}
                    >
                      {indexAction?.running && indexAction.action === "rebuild" ? t("indexRebuilding") : t("indexRebuild")}
                    </button>
                  </div>
                  <p className="text-[11px] text-[var(--text-muted)]">{t("indexRefreshDesc")}</p>
                </div>
              </section>

              {onEnterDevMode && (
                <section>
                  <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-2">{t("settingsDeveloper")}</h3>
                  <button
                    type="button"
                    className="text-xs px-3 py-1.5 bg-amber-700 hover:bg-amber-600 text-white rounded"
                    onClick={() => { onEnterDevMode(); onSettingsClose(); }}
                  >
                    {t("enterDevMode")}
                  </button>
                  <p className="text-xs text-[var(--text-muted)] mt-1">{t("devModeDesc")}</p>
                </section>
              )}
            </div>
          </div>
        </div>
      )}
    </>
  );
}
