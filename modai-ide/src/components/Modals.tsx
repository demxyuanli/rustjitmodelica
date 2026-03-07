import { t } from "../i18n";

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
          <div className="bg-surface-alt border border-border rounded-lg p-4 min-w-[320px] shadow-xl" onClick={(e) => e.stopPropagation()}>
            <div className="flex justify-between items-center mb-3">
              <span className="text-sm font-medium text-[var(--text)]">Settings</span>
              <button type="button" className="titlebar-btn px-2 py-1 text-sm rounded" onClick={onSettingsClose}>Close</button>
            </div>
            <div className="flex items-center gap-2 mb-2">
              <span className="text-xs text-[var(--text-muted)]">{t("theme")}:</span>
              <button type="button" className={`px-2 py-1 text-xs rounded ${theme === "dark" ? "bg-primary text-white" : "bg-gray-600 text-gray-300"}`} onClick={() => onThemeChange("dark")}>{t("themeDark")}</button>
              <button type="button" className={`px-2 py-1 text-xs rounded ${theme === "light" ? "bg-primary text-white" : "bg-gray-600 text-gray-300"}`} onClick={() => onThemeChange("light")}>{t("themeLight")}</button>
            </div>
            <p className="text-xs text-[var(--text-muted)]">Settings panel (placeholder)</p>
          </div>
        </div>
      )}
    </>
  );
}
