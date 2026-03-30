import { t } from "../../i18n";
import { SettingsRow } from "./settingsPrimitives";

export function SettingsShortcutsSection() {
  return (
    <section id="settings-group-shortcuts">
      <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionShortcuts")}</h3>
      <SettingsRow title={t("settingsKeyboard")} description={t("settingsKeyboardDesc")}>
        <div className="text-right text-xs space-y-1">
          <div className="flex justify-end gap-2"><span className="font-mono bg-[var(--surface)] px-2 py-0.5 rounded">{t("save")}</span><span className="text-[var(--text-muted)]">Ctrl+S</span></div>
          <div className="flex justify-end gap-2"><span className="font-mono bg-[var(--surface)] px-2 py-0.5 rounded">{t("toggleSidebar")}</span><span className="text-[var(--text-muted)]">Ctrl+B</span></div>
          <div className="flex justify-end gap-2"><span className="font-mono bg-[var(--surface)] px-2 py-0.5 rounded">{t("toggleBottomPanel")}</span><span className="text-[var(--text-muted)]">Ctrl+J</span></div>
        </div>
      </SettingsRow>
    </section>
  );
}
