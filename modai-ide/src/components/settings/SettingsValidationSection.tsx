import { t } from "../../i18n";
import type { AppSettingsForm } from "./settingsTypes";
import { SettingsRow } from "./settingsPrimitives";

export interface SettingsValidationSectionProps {
  appSettings: AppSettingsForm;
  onAppSettingsChange: (s: AppSettingsForm) => void;
}

export function SettingsValidationSection({
  appSettings,
  onAppSettingsChange,
}: SettingsValidationSectionProps) {
  const v = appSettings.validation ?? {};
  const tierRaw = (v.defaultTier ?? "analyze").trim().toLowerCase();
  const tier = ["parse", "flatten", "analyze", "full"].includes(tierRaw) ? tierRaw : "analyze";
  const eqModeRaw = (v.eqExpandParallelMode ?? "off").trim().toLowerCase();
  const eqMode = ["off", "guarded", "on"].includes(eqModeRaw) ? eqModeRaw : "off";

  return (
    <section id="settings-group-validation">
      <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">
        {t("settingsSectionValidation")}
      </h3>
      <SettingsRow
        title={t("settingsValidationDefaultTier")}
        description={t("settingsValidationDefaultTierDesc")}
      >
        <select
          value={tier}
          onChange={(e) =>
            onAppSettingsChange({
              ...appSettings,
              validation: { ...v, defaultTier: e.target.value },
            })
          }
          className="min-w-[200px] bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded"
        >
          <option value="parse">{t("settingsValidationTierParse")}</option>
          <option value="flatten">{t("settingsValidationTierFlatten")}</option>
          <option value="analyze">{t("settingsValidationTierAnalyze")}</option>
          <option value="full">{t("settingsValidationTierFull")}</option>
        </select>
      </SettingsRow>
      <SettingsRow
        title={t("settingsEqExpandParallelMode")}
        description={t("settingsEqExpandParallelModeDesc")}
      >
        <select
          value={eqMode}
          onChange={(e) =>
            onAppSettingsChange({
              ...appSettings,
              validation: { ...v, eqExpandParallelMode: e.target.value as "off" | "guarded" | "on" },
            })
          }
          className="min-w-[200px] bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded"
        >
          <option value="off">{t("settingsEqExpandParallelModeOff")}</option>
          <option value="guarded">{t("settingsEqExpandParallelModeGuarded")}</option>
          <option value="on">{t("settingsEqExpandParallelModeOn")}</option>
        </select>
      </SettingsRow>
    </section>
  );
}
