import { t } from "../../i18n";
import type { AppSettingsForm } from "./settingsTypes";
import { SettingsRow, SettingsSwitch } from "./settingsPrimitives";

export interface SettingsDocumentationSectionProps {
  appSettings: AppSettingsForm;
  onAppSettingsChange: (s: AppSettingsForm) => void;
}

export function SettingsDocumentationSection({ appSettings, onAppSettingsChange }: SettingsDocumentationSectionProps) {
  return (
    <section id="settings-group-documentation">
      <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionDocumentation")}</h3>
      <SettingsRow title={t("settingsHelpBaseUrl")} description={t("settingsHelpBaseUrlDesc")}>
        <input type="text" placeholder="https://..." value={appSettings.documentation?.helpBaseUrl ?? ""} onChange={(e) => onAppSettingsChange({ ...appSettings, documentation: { ...appSettings.documentation, helpBaseUrl: e.target.value } })} className="min-w-[200px] max-w-[320px] bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded" />
      </SettingsRow>
      <SettingsRow title={t("settingsShowWelcome")} description={t("settingsShowWelcomeDesc")}>
        <SettingsSwitch checked={appSettings.documentation?.showWelcomeOnFirstLaunch !== false} onChange={(v) => onAppSettingsChange({ ...appSettings, documentation: { ...appSettings.documentation, showWelcomeOnFirstLaunch: v } })} ariaLabel={t("settingsShowWelcome")} />
      </SettingsRow>
    </section>
  );
}
