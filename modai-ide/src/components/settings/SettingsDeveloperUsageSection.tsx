import { t } from "../../i18n";
import { SettingsRow } from "./settingsPrimitives";

export interface SettingsDeveloperUsageSectionProps {
  aiDailyUsed?: number;
  aiDailyLimit: number;
  onAiDailyReset?: () => void;
}

export function SettingsDeveloperUsageSection({
  aiDailyUsed,
  aiDailyLimit,
  onAiDailyReset,
}: SettingsDeveloperUsageSectionProps) {
  return (
    <section id="settings-group-developer">
      <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionDeveloper")}</h3>
      <SettingsRow title={t("settingsAiDailyUsage")} description={t("settingsAiDailyUsageDesc")}>
        <div className="flex items-center gap-3 flex-wrap">
          <span className="text-xs text-[var(--text)]">
            {t("dailyUsed")}: {aiDailyUsed ?? 0} / {aiDailyLimit}
          </span>
          {onAiDailyReset && (
            <button
              type="button"
              onClick={onAiDailyReset}
              className="px-3 py-1.5 text-xs rounded bg-[var(--surface)] border border-border hover:bg-white/10 text-[var(--text-muted)]"
            >
              {t("reset")}
            </button>
          )}
        </div>
      </SettingsRow>
    </section>
  );
}
