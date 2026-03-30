import { Check } from "lucide-react";
import { t } from "../../i18n";
import { SettingsRow } from "./settingsPrimitives";

export interface SettingsCompilerSectionProps {
  compilerExe: string;
  setCompilerExe: (v: string) => void;
  compilerArgs: string;
  setCompilerArgs: (v: string) => void;
  compilerConfigBanner: string | null;
  onSaveCompilerConfig: () => void;
}

export function SettingsCompilerSection({
  compilerExe,
  setCompilerExe,
  compilerArgs,
  setCompilerArgs,
  compilerConfigBanner,
  onSaveCompilerConfig,
}: SettingsCompilerSectionProps) {
  return (
    <section id="settings-group-compiler">
      <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionJitCompiler")}</h3>
      <div className="space-y-0">
        {compilerConfigBanner && (
          <div className="mb-3 px-3 py-2 rounded text-xs bg-green-900/30 text-green-300 border border-green-700">{compilerConfigBanner}</div>
        )}
        <SettingsRow title={t("settingsCompilerExePath")} description={t("settingsCompilerExeDesc")}>
          <div className="flex gap-2 items-center flex-wrap">
            <input type="text" placeholder={t("autoDetectPlaceholder")} value={compilerExe}
              onChange={(e) => setCompilerExe(e.target.value)}
              className="min-w-[200px] max-w-[320px] bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded font-mono" />
            <button type="button" onClick={onSaveCompilerConfig}
              className="p-2 rounded-md bg-primary hover:bg-blue-600 text-white"
              title={t("save")}
              aria-label={t("save")}>
              <Check size={16} />
            </button>
          </div>
        </SettingsRow>
        <SettingsRow title={t("settingsCompilerArgs")} description={t("settingsCompilerArgsDesc")}>
          <div className="flex gap-2 items-center flex-wrap">
            <input type="text" placeholder={t("compilerArgsPlaceholder")} value={compilerArgs}
              onChange={(e) => setCompilerArgs(e.target.value)}
              className="min-w-[200px] max-w-[320px] bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded font-mono" />
            <button type="button" onClick={onSaveCompilerConfig}
              className="p-2 rounded-md bg-primary hover:bg-blue-600 text-white"
              title={t("save")}
              aria-label={t("save")}>
              <Check size={16} />
            </button>
          </div>
        </SettingsRow>
      </div>
    </section>
  );
}
