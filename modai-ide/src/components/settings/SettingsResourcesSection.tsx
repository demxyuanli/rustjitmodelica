import { t } from "../../i18n";
import type { AppSettingsForm } from "./settingsTypes";
import { SettingsRow } from "./settingsPrimitives";

export interface SettingsResourcesSectionProps {
  appSettings: AppSettingsForm;
  onAppSettingsChange: (s: AppSettingsForm) => void;
}

export function SettingsResourcesSection({ appSettings, onAppSettingsChange }: SettingsResourcesSectionProps) {
  return (
    <section id="settings-group-resources">
      <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionResources")}</h3>
      <SettingsRow title={t("settingsLibrarySearchPaths")} description={t("settingsLibrarySearchPathsDesc")}>
        <input type="text" placeholder="C:\path\to\lib1;C:\path\to\lib2" value={(appSettings.resources?.librarySearchPaths ?? []).join("; ")} onChange={(e) => onAppSettingsChange({ ...appSettings, resources: { ...appSettings.resources, librarySearchPaths: e.target.value.split(";").map((s) => s.trim()).filter(Boolean) } })} className="min-w-[200px] max-w-[320px] bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded font-mono" />
      </SettingsRow>
    </section>
  );
}
