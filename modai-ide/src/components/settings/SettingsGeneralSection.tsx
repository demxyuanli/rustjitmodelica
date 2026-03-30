import { t } from "../../i18n";
import { PREFS_KEYS, writePref } from "../../utils/prefsConstants";
import { SettingsRow, SettingsSwitch } from "./settingsPrimitives";

type DefaultWorkspace = "modelica" | "component-library" | "compiler-iterate" | "regression";

export interface SettingsGeneralSectionProps {
  defaultWorkspace: DefaultWorkspace;
  onDefaultWorkspaceChange?: (v: DefaultWorkspace) => void;
  restoreLayout: boolean;
  onRestoreLayoutChange?: (v: boolean) => void;
  regressionAutoLoadOnOpen: boolean;
  onRegressionAutoLoadOnOpenChange: (v: boolean) => void;
  lang?: "en" | "zh";
  onLangChange?: (l: "en" | "zh") => void;
}

export function SettingsGeneralSection({
  defaultWorkspace,
  onDefaultWorkspaceChange,
  restoreLayout,
  onRestoreLayoutChange,
  regressionAutoLoadOnOpen,
  onRegressionAutoLoadOnOpenChange,
  lang,
  onLangChange,
}: SettingsGeneralSectionProps) {
  return (
    <section id="settings-group-general">
      <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">
        {t("settingsSectionGeneral")}
      </h3>
      <SettingsRow title={t("settingsDefaultWorkspace")} description={t("settingsDefaultWorkspaceDesc")}>
        <div className="flex rounded-md overflow-hidden border border-border">
          <button
            type="button"
            className={`px-3 py-1.5 text-xs ${defaultWorkspace === "modelica" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`}
            onClick={() => onDefaultWorkspaceChange?.("modelica")}
          >
            {t("workspaceModelica")}
          </button>
          <button
            type="button"
            className={`px-3 py-1.5 text-xs ${defaultWorkspace === "component-library" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`}
            onClick={() => onDefaultWorkspaceChange?.("component-library")}
          >
            {t("workspaceComponentLibrary")}
          </button>
          <button
            type="button"
            className={`px-3 py-1.5 text-xs ${defaultWorkspace === "compiler-iterate" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`}
            onClick={() => onDefaultWorkspaceChange?.("compiler-iterate")}
          >
            {t("workspaceCompilerIterate")}
          </button>
          <button
            type="button"
            className={`px-3 py-1.5 text-xs ${defaultWorkspace === "regression" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`}
            onClick={() => onDefaultWorkspaceChange?.("regression")}
          >
            {t("workspaceRegression")}
          </button>
        </div>
      </SettingsRow>
      <SettingsRow title={t("settingsRestoreLayout")} description={t("settingsRestoreLayoutDesc")}>
        <SettingsSwitch
          checked={restoreLayout}
          onChange={(v) => onRestoreLayoutChange?.(v)}
          ariaLabel={t("settingsRestoreLayout")}
        />
      </SettingsRow>
      <SettingsRow title={t("settingsRegressionAutoLoad")} description={t("settingsRegressionAutoLoadDesc")}>
        <SettingsSwitch
          checked={regressionAutoLoadOnOpen}
          onChange={(v) => {
            onRegressionAutoLoadOnOpenChange(v);
            writePref(PREFS_KEYS.regressionAutoLoadOnOpen, v ? "true" : "false");
          }}
          ariaLabel={t("settingsRegressionAutoLoad")}
        />
      </SettingsRow>
      <SettingsRow title={t("settingsLanguage")} description={t("settingsLanguageDesc")}>
        <div className="flex rounded-md overflow-hidden border border-border">
          <button
            type="button"
            className={`px-3 py-1.5 text-xs ${lang === "zh" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`}
            onClick={() => onLangChange?.("zh")}
          >
            {t("switchLanguageToChinese")}
          </button>
          <button
            type="button"
            className={`px-3 py-1.5 text-xs ${lang === "en" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`}
            onClick={() => onLangChange?.("en")}
          >
            {t("switchLanguageToEnglish")}
          </button>
        </div>
      </SettingsRow>
    </section>
  );
}
