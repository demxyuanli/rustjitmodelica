import { t } from "../../i18n";
import type { AppSettingsForm } from "./settingsTypes";
import { SettingsRow } from "./settingsPrimitives";

export interface SettingsExtensionsSectionProps {
  appSettings: AppSettingsForm;
  onAppSettingsChange: (s: AppSettingsForm) => void;
}

export function SettingsExtensionsSection({ appSettings, onAppSettingsChange }: SettingsExtensionsSectionProps) {
  return (
    <section id="settings-group-extensions">
      <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionExtensions")}</h3>
      <SettingsRow title={t("settingsPluginDir")} description={t("settingsPluginDirDesc")}>
        <input type="text" placeholder="" value={appSettings.extensions?.pluginDir ?? ""} onChange={(e) => onAppSettingsChange({ ...appSettings, extensions: { ...appSettings.extensions, pluginDir: e.target.value } })} className="min-w-[200px] max-w-[320px] bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded font-mono" />
      </SettingsRow>
      <SettingsRow title={t("settingsModelicaStdlibPath")} description={t("settingsModelicaStdlibPathDesc")}>
        <input type="text" placeholder="" value={appSettings.extensions?.modelicaStdlibPath ?? ""} onChange={(e) => onAppSettingsChange({ ...appSettings, extensions: { ...appSettings.extensions, modelicaStdlibPath: e.target.value } })} className="min-w-[200px] max-w-[320px] bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded font-mono" />
      </SettingsRow>
    </section>
  );
}
