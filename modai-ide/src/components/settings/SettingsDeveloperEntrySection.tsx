import { t } from "../../i18n";

export interface SettingsDeveloperEntrySectionProps {
  onEnterDevMode: () => void;
}

export function SettingsDeveloperEntrySection({ onEnterDevMode }: SettingsDeveloperEntrySectionProps) {
  return (
    <section id="settings-group-developer-entry">
      <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionDeveloper")}</h3>
      <div className="flex items-start justify-between gap-4 py-4">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <span className="text-sm font-medium text-[var(--text)]">{t("settingsDeveloper")}</span>
            <span className="text-[var(--text-muted)] opacity-70" title={t("devModeDesc")} aria-hidden="true">&#9432;</span>
          </div>
          <p className="text-xs text-[var(--text-muted)] mt-1">{t("devModeDesc")}</p>
        </div>
        <button type="button" className="text-xs px-3 py-1.5 bg-amber-700 hover:bg-amber-600 text-white rounded flex-shrink-0" onClick={onEnterDevMode}>{t("enterDevMode")}</button>
      </div>
    </section>
  );
}
