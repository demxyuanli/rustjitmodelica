import { t } from "../../i18n";
import type { EquationGraphMode } from "../../api/tauri";
import type { AppSettingsForm } from "./settingsTypes";
import { SettingsRow } from "./settingsPrimitives";

export interface SettingsDependencyGraphSectionProps {
  appSettings: AppSettingsForm;
  onAppSettingsChange: (s: AppSettingsForm) => void;
}

export function SettingsDependencyGraphSection({
  appSettings,
  onAppSettingsChange,
}: SettingsDependencyGraphSectionProps) {
  const dg = appSettings.dependencyGraph ?? {};
  const timeoutRaw = dg.fullTimeoutSec ?? 8;
  const timeoutClamped = Math.min(300, Math.max(1, Math.floor(Number.isFinite(timeoutRaw) ? timeoutRaw : 8)));
  const auto = dg.autoDowngradeFromFull ?? true;
  const target = dg.downgradeTarget === "top-level" ? "top-level" : "compact";
  const modeRaw = (dg.defaultGraphMode ?? "compact").trim().toLowerCase();
  const defaultMode: EquationGraphMode =
    modeRaw === "structural"
      ? "structural"
      : modeRaw === "full"
        ? "full"
        : modeRaw === "top-level"
          ? "top-level"
          : "compact";
  const preferStructural = dg.preferStructuralFirst ?? false;

  return (
    <section id="settings-group-dependency-graph">
      <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">
        {t("settingsSectionDependencyGraph")}
      </h3>
      <SettingsRow title={t("settingsDependencyGraphAutoDowngrade")} description={t("settingsDependencyGraphAutoDowngradeDesc")}>
        <label className="flex items-center gap-2 text-sm text-[var(--text)]">
          <input
            type="checkbox"
            checked={auto}
            onChange={(e) =>
              onAppSettingsChange({
                ...appSettings,
                dependencyGraph: { ...dg, autoDowngradeFromFull: e.target.checked },
              })
            }
          />
          <span>{t("settingsDependencyGraphAutoDowngradeLabel")}</span>
        </label>
      </SettingsRow>
      <SettingsRow title={t("settingsDependencyGraphFullTimeout")} description={t("settingsDependencyGraphFullTimeoutDesc")}>
        <input
          type="number"
          min={1}
          max={300}
          step={1}
          disabled={!auto}
          value={timeoutClamped}
          onChange={(e) => {
            const v = parseInt(e.target.value, 10);
            const next = Number.isFinite(v) ? Math.min(300, Math.max(1, v)) : 8;
            onAppSettingsChange({
              ...appSettings,
              dependencyGraph: { ...dg, fullTimeoutSec: next },
            });
          }}
          className="w-24 bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded font-mono disabled:opacity-50"
        />
      </SettingsRow>
      <SettingsRow title={t("settingsDependencyGraphDowngradeTarget")} description={t("settingsDependencyGraphDowngradeTargetDesc")}>
        <select
          value={target}
          disabled={!auto}
          onChange={(e) =>
            onAppSettingsChange({
              ...appSettings,
              dependencyGraph: {
                ...dg,
                downgradeTarget: e.target.value === "top-level" ? "top-level" : "compact",
              },
            })
          }
          className="min-w-[160px] bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded disabled:opacity-50"
        >
          <option value="compact">{t("dependencyGraphModeCompact")}</option>
          <option value="top-level">{t("dependencyGraphModeTopLevel")}</option>
        </select>
      </SettingsRow>
      <SettingsRow title={t("settingsDependencyGraphDefaultMode")} description={t("settingsDependencyGraphDefaultModeDesc")}>
        <select
          value={defaultMode}
          onChange={(e) =>
            onAppSettingsChange({
              ...appSettings,
              dependencyGraph: {
                ...dg,
                defaultGraphMode: e.target.value as EquationGraphMode,
              },
            })
          }
          className="min-w-[180px] bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded"
        >
          <option value="structural">{t("dependencyGraphModeStructural")}</option>
          <option value="compact">{t("dependencyGraphModeCompact")}</option>
          <option value="top-level">{t("dependencyGraphModeTopLevel")}</option>
          <option value="full">{t("dependencyGraphModeFull")}</option>
        </select>
      </SettingsRow>
      <SettingsRow
        title={t("settingsDependencyGraphPreferStructuralFirst")}
        description={t("settingsDependencyGraphPreferStructuralFirstDesc")}
      >
        <label className="flex items-center gap-2 text-sm text-[var(--text)]">
          <input
            type="checkbox"
            checked={preferStructural}
            aria-label={t("settingsDependencyGraphPreferStructuralFirst")}
            onChange={(e) =>
              onAppSettingsChange({
                ...appSettings,
                dependencyGraph: { ...dg, preferStructuralFirst: e.target.checked },
              })
            }
          />
        </label>
      </SettingsRow>
    </section>
  );
}
