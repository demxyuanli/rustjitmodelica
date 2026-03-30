import { FolderOpen } from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { t } from "../../i18n";
import type { AppSettingsForm } from "./settingsTypes";
import { SettingsRow, SettingsSwitch } from "./settingsPrimitives";

export interface SettingsStorageSectionProps {
  appDataRoot?: string;
  appSettings?: AppSettingsForm;
  onAppSettingsChange?: (s: AppSettingsForm) => void;
}

export function SettingsStorageSection({ appDataRoot, appSettings, onAppSettingsChange }: SettingsStorageSectionProps) {
  return (
    <section id="settings-group-storage">
      <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionStorage")}</h3>
      <SettingsRow title={t("settingsAppDataRoot")} description={t("settingsAppDataRootDesc")}>
        <div className="flex items-center gap-2">
          <span className="text-xs font-mono text-[var(--text-muted)] max-w-[240px] truncate" title={appDataRoot}>{appDataRoot ?? "—"}</span>
          {appDataRoot && (
            <button type="button" onClick={() => openPath(appDataRoot)} className="p-2 rounded-md bg-[var(--surface)] border border-border hover:bg-white/10 text-[var(--text-muted)]" title={t("openFolder")} aria-label={t("openFolder")}>
              <FolderOpen size={16} />
            </button>
          )}
        </div>
      </SettingsRow>
      {onAppSettingsChange && appSettings && (
        <SettingsRow title={t("settingsAllowProjectWrites")} description={t("settingsAllowProjectWritesDesc")}>
          <SettingsSwitch checked={(appSettings.storage?.allowProjectWrites ?? true) !== false} onChange={(v) => onAppSettingsChange({ ...appSettings, storage: { ...appSettings.storage, allowProjectWrites: v } })} ariaLabel={t("settingsAllowProjectWrites")} />
        </SettingsRow>
      )}
    </section>
  );
}
