import React, { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openPath } from "@tauri-apps/plugin-opener";
import { getApiKey, setApiKey as setApiKeyCommand } from "../api/tauri";
import { t } from "../i18n";
import { DiagramColorSchemePreview } from "./DiagramColorSchemePreview";
import {
  DEFAULT_CONNECTOR_KEYS,
  DIAGRAM_COLOR_SCHEMES,
  setSingleColorOverride,
  clearColorOverrides,
  hasColorOverrides,
  type DiagramColorScheme,
} from "../utils/diagramColorSchemes";
import { AppIcon } from "./Icon";
import type { AppIconName } from "./Icon";

const SETTINGS_GROUPS: { id: string; labelKey: string; icon: AppIconName; dividerAfter?: boolean }[] = [
  { id: "general", labelKey: "settingsSectionGeneral", icon: "explorer" },
  { id: "appearance", labelKey: "settingsSectionAppearance", icon: "variables" },
  { id: "shortcuts", labelKey: "settingsSectionShortcuts", icon: "columns" },
  { id: "account", labelKey: "settingsSectionAccount", icon: "user" },
  { id: "compiler", labelKey: "settingsSectionJitCompiler", icon: "simSettings" },
  { id: "storage", labelKey: "settingsSectionStorage", icon: "index" },
  { id: "codebase", labelKey: "settingsSectionCodebase", icon: "index" },
  { id: "resources", labelKey: "settingsSectionResources", icon: "library" },
  { id: "documentation", labelKey: "settingsSectionDocumentation", icon: "columns" },
  { id: "extensions", labelKey: "settingsSectionExtensions", icon: "iterate" },
  { id: "developer", labelKey: "settingsSectionDeveloper", icon: "ai", dividerAfter: true },
];

const CONNECTOR_LABEL_KEYS: Record<string, string> = {
  mechanical: "diagramConnectorMechanical",
  electrical: "diagramConnectorElectrical",
  thermal: "diagramConnectorThermal",
  fluid: "diagramConnectorFluid",
  signal_input: "diagramConnectorSignalInput",
  signal_output: "diagramConnectorSignalOutput",
};

export interface IndexActionState {
  running: boolean;
  action: "refresh" | "rebuild" | null;
  done: number;
  total: number;
}

export type DefaultWorkspace = "modelica" | "component-library" | "compiler-iterate";

export interface AppSettingsForm {
  storage?: { indexPathPolicy?: string; allowProjectWrites?: boolean };
  resources?: { librarySearchPaths?: string[]; packageCacheDir?: string };
  documentation?: { helpBaseUrl?: string; showWelcomeOnFirstLaunch?: boolean };
  extensions?: { pluginDir?: string; modelicaStdlibPath?: string };
}

export interface SettingsContentProps {
  theme: "dark" | "light";
  onThemeChange: (theme: "dark" | "light") => void;
  fontUi?: "chinese" | "code";
  onFontUiChange?: (v: "chinese" | "code") => void;
  fontSizePercent?: 90 | 100 | 110 | 120;
  onFontSizePercentChange?: (v: 90 | 100 | 110 | 120) => void;
  uiColorScheme?: "default" | "classic";
  onUiColorSchemeChange?: (v: "default" | "classic") => void;
  diagramSchemeId?: string;
  diagramScheme?: DiagramColorScheme;
  onDiagramSchemeChange?: (id: string) => void;
  indexFileCount?: number;
  indexSymbolCount?: number;
  indexState?: "idle" | "building" | "ready" | null;
  indexAction?: IndexActionState;
  onIndexRefresh?: () => void;
  onIndexRebuild?: () => void;
  onEnterDevMode?: () => void;
  aiModel?: string;
  onAiModelChange?: (model: string) => void;
  aiDailyUsed?: number;
  aiDailyLimit?: number;
  onAiDailyReset?: () => void;
  onClose?: () => void;
  defaultWorkspace?: DefaultWorkspace;
  onDefaultWorkspaceChange?: (v: DefaultWorkspace) => void;
  restoreLayout?: boolean;
  onRestoreLayoutChange?: (v: boolean) => void;
  lang?: "en" | "zh";
  onLangChange?: (l: "en" | "zh") => void;
  panelHeaderHeight?: number;
  onPanelHeaderHeightChange?: (v: number) => void;
  toolbarBtnSize?: number;
  onToolbarBtnSizeChange?: (v: number) => void;
  toolbarGap?: number;
  onToolbarGapChange?: (v: number) => void;
  appDataRoot?: string;
  appSettings?: AppSettingsForm;
  onAppSettingsChange?: (s: AppSettingsForm) => void;
}

function SettingsRow({
  title,
  description,
  children,
}: { title: string; description?: string; children: React.ReactNode }) {
  return (
    <div className="flex items-start justify-between gap-4 py-4 first:pt-0 last:pb-0 border-b border-[var(--border)] last:border-b-0">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <span className="text-sm font-medium text-[var(--text)]">{title}</span>
          {description && <span className="text-[var(--text-muted)] opacity-70" title={description} aria-hidden="true">&#9432;</span>}
        </div>
        {description && <p className="text-xs text-[var(--text-muted)] mt-1">{description}</p>}
      </div>
      <div className="flex-shrink-0 flex items-center gap-2">{children}</div>
    </div>
  );
}

export function SettingsContent({
  theme,
  onThemeChange,
  fontUi: fontUiProp,
  onFontUiChange,
  fontSizePercent: fontSizePercentProp,
  onFontSizePercentChange,
  uiColorScheme,
  onUiColorSchemeChange,
  diagramSchemeId,
  diagramScheme,
  onDiagramSchemeChange,
  indexFileCount = 0,
  indexSymbolCount = 0,
  indexState,
  indexAction,
  onIndexRefresh,
  onIndexRebuild,
  onEnterDevMode,
  aiModel,
  onAiModelChange,
  aiDailyUsed,
  aiDailyLimit,
  onAiDailyReset,
  onClose,
  defaultWorkspace = "modelica",
  onDefaultWorkspaceChange,
  restoreLayout = true,
  onRestoreLayoutChange,
  lang = "zh",
  onLangChange,
  panelHeaderHeight,
  onPanelHeaderHeightChange,
  toolbarBtnSize,
  onToolbarBtnSizeChange,
  toolbarGap,
  onToolbarGapChange,
  appDataRoot,
  appSettings,
  onAppSettingsChange,
}: SettingsContentProps) {
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [apiKeySaved, setApiKeySaved] = useState(false);
  const [apiKeyBanner, setApiKeyBanner] = useState<string | null>(null);

  const [compilerExe, setCompilerExe] = useState("");
  const [compilerArgs, setCompilerArgs] = useState("");
  const [compilerConfigBanner, setCompilerConfigBanner] = useState<string | null>(null);

  const readFontUiFromStorage = (): "chinese" | "code" =>
    (typeof localStorage !== "undefined" && localStorage.getItem("modai-font-ui") === "code") ? "code" : "chinese";
  const readFontSizePercentFromStorage = (): 90 | 100 | 110 | 120 => {
    if (typeof localStorage === "undefined") return 100;
    const s = localStorage.getItem("modai-font-size-percent");
    const n = s ? parseInt(s, 10) : 100;
    return [90, 100, 110, 120].includes(n) ? (n as 90 | 100 | 110 | 120) : 100;
  };
  const [localFontUi, setLocalFontUi] = useState<"chinese" | "code">(readFontUiFromStorage);
  const [localFontSizePercent, setLocalFontSizePercent] = useState<90 | 100 | 110 | 120>(readFontSizePercentFromStorage);
  const fontUi = fontUiProp ?? localFontUi;
  const fontSizePercent = fontSizePercentProp ?? localFontSizePercent;

  const handleFontUiChange = useCallback((v: "chinese" | "code") => {
    try { localStorage.setItem("modai-font-ui", v); } catch { /* ignore */ }
    if (onFontUiChange) onFontUiChange(v); else setLocalFontUi(v);
    window.dispatchEvent(new CustomEvent("modai-font-ui-change"));
  }, [onFontUiChange]);

  const handleFontSizePercentChange = useCallback((v: 90 | 100 | 110 | 120) => {
    try { localStorage.setItem("modai-font-size-percent", String(v)); } catch { /* ignore */ }
    if (onFontSizePercentChange) onFontSizePercentChange(v); else setLocalFontSizePercent(v);
    window.dispatchEvent(new CustomEvent("modai-font-size-change"));
  }, [onFontSizePercentChange]);

  useEffect(() => {
    invoke<{ exe?: string; args?: string[] } | null>("get_compiler_config")
      .then((c) => {
        if (c) {
          setCompilerExe(c.exe ?? "");
          setCompilerArgs(Array.isArray(c.args) ? c.args.join(" ") : "");
        }
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    if (compilerConfigBanner) {
      const tm = setTimeout(() => setCompilerConfigBanner(null), 3000);
      return () => clearTimeout(tm);
    }
  }, [compilerConfigBanner]);

  const handleSaveCompilerConfig = useCallback(async () => {
    try {
      const args = compilerArgs.trim() ? compilerArgs.trim().split(/\s+/).filter(Boolean) : [];
      await invoke("set_compiler_config", { config: { exe: compilerExe.trim(), args } });
      setCompilerConfigBanner(t("saved"));
    } catch (e) {
      setCompilerConfigBanner(String(e));
    }
  }, [compilerExe, compilerArgs]);

  useEffect(() => {
    getApiKey()
      .then(() => setApiKeySaved(true))
      .catch(() => setApiKeySaved(false));
  }, []);

  useEffect(() => {
    if (apiKeyBanner) {
      const tm = setTimeout(() => setApiKeyBanner(null), 3000);
      return () => clearTimeout(tm);
    }
  }, [apiKeyBanner]);

  const handleSaveApiKey = useCallback(async () => {
    if (!apiKeyInput.trim()) return;
    try {
      await setApiKeyCommand(apiKeyInput.trim());
      setApiKeySaved(true);
      setApiKeyInput("");
      setApiKeyBanner(t("apiKeySaveSuccess"));
    } catch (e) {
      setApiKeyBanner(String(e));
    }
  }, [apiKeyInput]);

  const handleClearApiKey = useCallback(async () => {
    try {
      await setApiKeyCommand("");
      setApiKeySaved(false);
      setApiKeyBanner(t("apiKeyClearSuccess"));
    } catch (e) {
      setApiKeyBanner(String(e));
    }
  }, []);

  const indexPct = indexState === "ready" && indexFileCount > 0 ? 100 : (indexAction?.total ? Math.round((indexAction.done / indexAction.total) * 100) : 0);

  const showDeveloperGroup = Boolean(onAiModelChange && aiDailyLimit != null);
  const [groupFilter, setGroupFilter] = useState("");
  const visibleGroups = SETTINGS_GROUPS.filter((g) => {
    if (g.id === "developer" && !showDeveloperGroup) return false;
    if (!groupFilter.trim()) return true;
    return t(g.labelKey).toLowerCase().includes(groupFilter.trim().toLowerCase());
  });

  const [activeGroupId, setActiveGroupId] = useState<string>(SETTINGS_GROUPS[0].id);
  const effectiveGroupId = visibleGroups.some((g) => g.id === activeGroupId)
    ? activeGroupId
    : (visibleGroups[0]?.id ?? activeGroupId);

  useEffect(() => {
    if (!visibleGroups.some((g) => g.id === activeGroupId) && visibleGroups[0])
      setActiveGroupId(visibleGroups[0].id);
  }, [visibleGroups, activeGroupId]);

  const selectGroup = useCallback((id: string) => setActiveGroupId(id), []);

  return (
    <div className="flex flex-1 min-h-0 flex-col text-[var(--text)]">
      {onClose && (
        <div className="panel-header-bar shrink-0 flex items-center border-b border-[var(--border)]">
          <button
            type="button"
            onClick={onClose}
            className="flex items-center gap-1.5 px-2 py-1.5 text-sm text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-[var(--surface-hover)] rounded"
          >
            <span aria-hidden="true">&#8592;</span>
            <span>{t("closeSettings")}</span>
          </button>
        </div>
      )}
      <div className="flex flex-1 min-h-0 min-w-0">
      <aside className="shrink-0 w-[200px] py-4 pr-2 border-r border-[var(--border)] flex flex-col bg-[var(--surface-muted)]/50">
        <div className="px-2 pb-3">
          <input
            type="text"
            placeholder={t("settingsGroupSearchPlaceholder")}
            value={groupFilter}
            onChange={(e) => setGroupFilter(e.target.value)}
            className="w-full rounded-md border border-[var(--border)] bg-[var(--surface)] px-2.5 py-1.5 text-xs text-[var(--text)] placeholder:text-[var(--text-muted)] focus:outline-none focus:ring-1 focus:ring-[var(--primary)]"
          />
        </div>
        <nav className="flex flex-col gap-0.5">
          {visibleGroups.map((group) => (
            <React.Fragment key={group.id}>
              <button
                type="button"
                onClick={() => selectGroup(group.id)}
                className={`flex items-center gap-2.5 w-full text-left px-3 py-2 rounded-md text-sm transition-colors ${effectiveGroupId === group.id ? "bg-[var(--surface-elevated)] text-[var(--text)]" : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"}`}
              >
                <AppIcon name={group.icon} className="shrink-0 w-4 h-4" aria-hidden />
                <span className="truncate">{t(group.labelKey)}</span>
              </button>
              {group.dividerAfter && <hr className="my-1 border-0 border-t border-[var(--border)]" />}
            </React.Fragment>
          ))}
        </nav>
      </aside>
      <main className="flex-1 min-w-0 overflow-auto py-4 pl-6">
        <div className="max-w-2xl">
      {effectiveGroupId === "general" && (
      <section id="settings-group-general">
        <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionGeneral")}</h3>
        <SettingsRow title={t("settingsDefaultWorkspace")} description={t("settingsDefaultWorkspaceDesc")}>
          <div className="flex rounded-md overflow-hidden border border-border">
            <button type="button" className={`px-3 py-1.5 text-xs ${defaultWorkspace === "modelica" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onDefaultWorkspaceChange?.("modelica")}>{t("workspaceModelica")}</button>
            <button type="button" className={`px-3 py-1.5 text-xs ${defaultWorkspace === "component-library" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onDefaultWorkspaceChange?.("component-library")}>{t("workspaceComponentLibrary")}</button>
            <button type="button" className={`px-3 py-1.5 text-xs ${defaultWorkspace === "compiler-iterate" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onDefaultWorkspaceChange?.("compiler-iterate")}>{t("workspaceCompilerIterate")}</button>
          </div>
        </SettingsRow>
        <SettingsRow title={t("settingsRestoreLayout")} description={t("settingsRestoreLayoutDesc")}>
          <div className="flex rounded-md overflow-hidden border border-border">
            <button type="button" className={`px-3 py-1.5 text-xs ${restoreLayout ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onRestoreLayoutChange?.(true)}>{t("yes")}</button>
            <button type="button" className={`px-3 py-1.5 text-xs ${!restoreLayout ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onRestoreLayoutChange?.(false)}>{t("no")}</button>
          </div>
        </SettingsRow>
        <SettingsRow title={t("settingsLanguage")} description={t("settingsLanguageDesc")}>
          <div className="flex rounded-md overflow-hidden border border-border">
            <button type="button" className={`px-3 py-1.5 text-xs ${lang === "zh" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onLangChange?.("zh")}>{t("switchLanguageToChinese")}</button>
            <button type="button" className={`px-3 py-1.5 text-xs ${lang === "en" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onLangChange?.("en")}>{t("switchLanguageToEnglish")}</button>
          </div>
        </SettingsRow>
      </section>
      )}

      {effectiveGroupId === "account" && (
      <section id="settings-group-account">
        <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionAccount")}</h3>
        <div className="space-y-0">
          {apiKeyBanner && (
            <div className="mb-3 px-3 py-2 rounded text-xs bg-green-900/30 text-green-300 border border-green-700">{apiKeyBanner}</div>
          )}
          <SettingsRow title={t("settingsApiKey")} description={t("settingsApiKeyDesc")}>
            {apiKeySaved ? (
              <div className="flex items-center gap-2">
                <span className="text-xs text-green-400">{t("apiKeySaved")}</span>
                <button type="button" onClick={handleClearApiKey}
                  className="px-3 py-1.5 text-xs rounded bg-[var(--surface)] border border-border hover:bg-white/10 text-[var(--text-muted)]">
                  {t("apiKeyClear")}
                </button>
              </div>
            ) : (
              <div className="flex gap-2 items-center">
                <input type="password" placeholder={t("apiKeyPlaceholder")} value={apiKeyInput}
                  onChange={(e) => setApiKeyInput(e.target.value)}
                  onKeyDown={(e) => { if (e.key === "Enter") handleSaveApiKey(); }}
                  className="w-40 bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded" />
                <button type="button" onClick={handleSaveApiKey} disabled={!apiKeyInput.trim()}
                  className="px-3 py-1.5 bg-primary hover:bg-blue-600 text-sm rounded text-white disabled:opacity-40">
                  {t("save")}
                </button>
              </div>
            )}
          </SettingsRow>
        </div>
      </section>
      )}

      {effectiveGroupId === "compiler" && (
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
              <button type="button" onClick={handleSaveCompilerConfig}
                className="px-3 py-1.5 bg-primary hover:bg-blue-600 text-sm rounded text-white">
                {t("save")}
              </button>
            </div>
          </SettingsRow>
          <SettingsRow title={t("settingsCompilerArgs")} description={t("settingsCompilerArgsDesc")}>
            <div className="flex gap-2 items-center flex-wrap">
              <input type="text" placeholder={t("compilerArgsPlaceholder")} value={compilerArgs}
                onChange={(e) => setCompilerArgs(e.target.value)}
                className="min-w-[200px] max-w-[320px] bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded font-mono" />
              <button type="button" onClick={handleSaveCompilerConfig}
                className="px-3 py-1.5 bg-primary hover:bg-blue-600 text-sm rounded text-white">
                {t("save")}
              </button>
            </div>
          </SettingsRow>
        </div>
      </section>
      )}

      {effectiveGroupId === "developer" && onAiModelChange && aiDailyLimit != null && (
        <section id="settings-group-developer">
          <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionDeveloper")}</h3>
          <SettingsRow title={t("settingsAiModel")} description={t("settingsAiModelDesc")}>
            <select
              value={aiModel || "deepseek-chat"}
              onChange={(e) => onAiModelChange(e.target.value)}
              className="bg-[var(--surface)] border border-border px-2.5 py-1.5 text-xs rounded text-[var(--text)]"
            >
              <option value="deepseek-chat">deepseek-chat</option>
            </select>
            <div className="text-xs text-[var(--text-muted)]">
              {t("dailyUsed")}: {aiDailyUsed ?? 0} / {aiDailyLimit}
            </div>
            {onAiDailyReset && (
              <button
                type="button"
                onClick={onAiDailyReset}
                className="px-3 py-1.5 text-xs rounded bg-[var(--surface)] border border-border hover:bg-white/10 text-[var(--text-muted)]"
              >
                {t("reset")}
              </button>
            )}
          </SettingsRow>
        </section>
      )}

      {effectiveGroupId === "appearance" && (
      <section id="settings-group-appearance">
        <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionAppearance")}</h3>
        <SettingsRow title={t("settingsAppearance")} description={t("settingsAppearanceDesc")}>
          <div className="flex rounded-md overflow-hidden border border-border">
            <button type="button" className={`px-3 py-1.5 text-xs ${theme === "dark" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onThemeChange("dark")}>{t("themeDark")}</button>
            <button type="button" className={`px-3 py-1.5 text-xs ${theme === "light" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onThemeChange("light")}>{t("themeLight")}</button>
          </div>
        </SettingsRow>
        <SettingsRow title={t("settingsFontUi")} description={t("settingsFontUiDesc")}>
          <div className="flex rounded-md overflow-hidden border border-border">
            <button type="button" className={`px-3 py-1.5 text-xs ${fontUi === "chinese" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => handleFontUiChange("chinese")}>{t("fontUiChinese")}</button>
            <button type="button" className={`px-3 py-1.5 text-xs ${fontUi === "code" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => handleFontUiChange("code")}>{t("fontUiCode")}</button>
          </div>
        </SettingsRow>
        <SettingsRow title={t("settingsFontSize")} description={t("settingsFontSizeDesc")}>
          <div className="flex rounded-md overflow-hidden border border-border">
            <button type="button" className={`px-3 py-1.5 text-xs ${fontSizePercent === 90 ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => handleFontSizePercentChange(90)}>{t("fontSize90")}</button>
            <button type="button" className={`px-3 py-1.5 text-xs ${fontSizePercent === 100 ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => handleFontSizePercentChange(100)}>{t("fontSize100")}</button>
            <button type="button" className={`px-3 py-1.5 text-xs ${fontSizePercent === 110 ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => handleFontSizePercentChange(110)}>{t("fontSize110")}</button>
            <button type="button" className={`px-3 py-1.5 text-xs ${fontSizePercent === 120 ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => handleFontSizePercentChange(120)}>{t("fontSize120")}</button>
          </div>
        </SettingsRow>
        {onUiColorSchemeChange != null && uiColorScheme != null && (
          <SettingsRow title={t("settingsUiColorScheme")} description={t("settingsUiColorSchemeDesc")}>
            <div className="flex rounded-md overflow-hidden border border-border">
              <button type="button" className={`px-3 py-1.5 text-xs ${uiColorScheme === "default" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onUiColorSchemeChange("default")}>{t("uiColorDefault")}</button>
              <button type="button" className={`px-3 py-1.5 text-xs ${uiColorScheme === "classic" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onUiColorSchemeChange("classic")}>{t("uiColorClassic")}</button>
            </div>
          </SettingsRow>
        )}
        {onPanelHeaderHeightChange != null && panelHeaderHeight != null && (
          <SettingsRow title={t("settingsLayoutPanelHeaderHeight")} description={t("settingsLayoutPanelHeaderHeightDesc")}>
            <div className="flex rounded-md overflow-hidden border border-border">
              {[28, 32, 36].map((px) => (
                <button key={px} type="button" className={`px-3 py-1.5 text-xs ${panelHeaderHeight === px ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onPanelHeaderHeightChange(px)}>{px}px</button>
              ))}
            </div>
          </SettingsRow>
        )}
        {onToolbarBtnSizeChange != null && toolbarBtnSize != null && (
          <SettingsRow title={t("settingsLayoutToolbarBtnSize")} description={t("settingsLayoutToolbarBtnSizeDesc")}>
            <div className="flex rounded-md overflow-hidden border border-border">
              {[24, 26, 28].map((px) => (
                <button key={px} type="button" className={`px-3 py-1.5 text-xs ${toolbarBtnSize === px ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onToolbarBtnSizeChange(px)}>{px}px</button>
              ))}
            </div>
          </SettingsRow>
        )}
        {onToolbarGapChange != null && toolbarGap != null && (
          <SettingsRow title={t("settingsLayoutToolbarGap")} description={t("settingsLayoutToolbarGapDesc")}>
            <div className="flex rounded-md overflow-hidden border border-border">
              {[6, 8, 10].map((px) => (
                <button key={px} type="button" className={`px-3 py-1.5 text-xs ${toolbarGap === px ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onToolbarGapChange(px)}>{px}px</button>
              ))}
            </div>
          </SettingsRow>
        )}
        {onDiagramSchemeChange != null && diagramSchemeId != null && (
          <div className="flex flex-col gap-2 py-4 border-b border-[var(--border)] last:border-b-0">
            <div className="min-w-0">
              <div className="flex items-center gap-1.5">
                <span className="text-sm font-medium text-[var(--text)]">{t("settingsDiagramColors")}</span>
                <span className="text-[var(--text-muted)] opacity-70" title={t("settingsDiagramColorsDesc")} aria-hidden="true">&#9432;</span>
              </div>
              <p className="text-xs text-[var(--text-muted)] mt-1">{t("settingsDiagramColorsDesc")}</p>
            </div>
            <div className="flex flex-wrap gap-3 justify-start">
              {DIAGRAM_COLOR_SCHEMES.map((scheme) => (
                <button
                  key={scheme.id}
                  type="button"
                  onClick={() => onDiagramSchemeChange(scheme.id)}
                  className={`rounded-lg border-2 p-1.5 transition-colors flex-shrink-0 ${diagramSchemeId === scheme.id ? "border-[var(--primary)] ring-2 ring-[var(--primary)]/30 bg-[var(--primary)]/5" : "border-[var(--border)] hover:border-[var(--text-muted)] hover:bg-white/5"}`}
                >
                  <DiagramColorSchemePreview scheme={scheme} compact />
                </button>
              ))}
            </div>
            {diagramSchemeId && diagramScheme && (
              <div className="mt-4 pt-4 border-t border-[var(--border)] space-y-4">
                <div className="flex items-center justify-between gap-2">
                  <span className="text-sm font-medium text-[var(--text)]">{t("settingsDiagramEditColors")}</span>
                  {hasColorOverrides(diagramSchemeId) && (
                    <button
                      type="button"
                      onClick={() => clearColorOverrides(diagramSchemeId)}
                      className="px-2.5 py-1 text-xs rounded border border-[var(--border)] bg-[var(--surface)] text-[var(--text-muted)] hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
                    >
                      {t("settingsDiagramResetScheme")}
                    </button>
                  )}
                </div>
                <div>
                  <div className="text-xs font-medium text-[var(--text-muted)] mb-2">{t("settingsDiagramConnectorColors")}</div>
                  <div className="flex flex-wrap gap-3">
                    {DEFAULT_CONNECTOR_KEYS.map((key) => (
                      <div key={key} className="flex items-center gap-2">
                        <label className="text-xs text-[var(--text)] whitespace-nowrap">{t(CONNECTOR_LABEL_KEYS[key] ?? key)}</label>
                        <input
                          type="color"
                          value={diagramScheme.connectorColors[key] ?? "#888"}
                          onChange={(e) => setSingleColorOverride(diagramSchemeId, "connectorColors", key, e.target.value)}
                          className="w-8 h-8 rounded border border-[var(--border)] cursor-pointer bg-[var(--surface)]"
                          title={t(CONNECTOR_LABEL_KEYS[key] ?? key)}
                        />
                      </div>
                    ))}
                  </div>
                </div>
                <div>
                  <div className="text-xs font-medium text-[var(--text-muted)] mb-2">{t("settingsDiagramChartPaletteLight")}</div>
                  <div className="flex flex-wrap gap-2">
                    {(diagramScheme.chartPaletteLight ?? []).map((color, i) => (
                      <div key={`light-${i}`} className="flex items-center gap-1">
                        <span className="text-[10px] text-[var(--text-muted)] w-3">{i + 1}</span>
                        <input
                          type="color"
                          value={color}
                          onChange={(e) => setSingleColorOverride(diagramSchemeId, "chartPaletteLight", i, e.target.value)}
                          className="w-8 h-8 rounded border border-[var(--border)] cursor-pointer bg-[var(--surface)]"
                          title={`${t("settingsDiagramChartPaletteLight")} ${i + 1}`}
                        />
                      </div>
                    ))}
                  </div>
                </div>
                <div>
                  <div className="text-xs font-medium text-[var(--text-muted)] mb-2">{t("settingsDiagramChartPaletteDark")}</div>
                  <div className="flex flex-wrap gap-2">
                    {(diagramScheme.chartPaletteDark ?? []).map((color, i) => (
                      <div key={`dark-${i}`} className="flex items-center gap-1">
                        <span className="text-[10px] text-[var(--text-muted)] w-3">{i + 1}</span>
                        <input
                          type="color"
                          value={color}
                          onChange={(e) => setSingleColorOverride(diagramSchemeId, "chartPaletteDark", i, e.target.value)}
                          className="w-8 h-8 rounded border border-[var(--border)] cursor-pointer bg-[var(--surface)]"
                          title={`${t("settingsDiagramChartPaletteDark")} ${i + 1}`}
                        />
                      </div>
                    ))}
                  </div>
                </div>
                <div>
                  <div className="text-xs font-medium text-[var(--text-muted)] mb-2">{t("settingsDiagramPrimaryColor")}</div>
                  <div className="flex items-center gap-2">
                    <input
                      type="color"
                      value={diagramScheme.diagramPrimary ?? "#3b82f6"}
                      onChange={(e) => setSingleColorOverride(diagramSchemeId, "diagramPrimary", 0, e.target.value)}
                      className="w-8 h-8 rounded border border-[var(--border)] cursor-pointer bg-[var(--surface)]"
                      title={t("settingsDiagramPrimaryColor")}
                    />
                    <span className="text-xs text-[var(--text-muted)]">{t("settingsDiagramPrimaryColor")}</span>
                  </div>
                </div>
              </div>
            )}
          </div>
        )}
      </section>
      )}

      {effectiveGroupId === "shortcuts" && (
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
      )}

      {effectiveGroupId === "codebase" && (
      <section id="settings-group-codebase">
        <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionCodebase")}</h3>
        <div className="space-y-0">
          <div className="flex items-start justify-between gap-4 py-4 border-b border-[var(--border)]">
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-1.5">
                <span className="text-sm font-medium text-[var(--text)]">{t("settingsIndex")}</span>
                <span className="text-[var(--text-muted)] opacity-70" title={t("settingsIndexDesc")} aria-hidden="true">&#9432;</span>
              </div>
              <p className="text-xs text-[var(--text-muted)] mt-1">{t("settingsIndexDesc")}</p>
              {indexAction?.running && (
                <div className="mt-2 space-y-1">
                  <div className="flex items-center justify-between text-[11px] text-[var(--text-muted)]">
                    <span>{indexAction.action === "rebuild" ? t("indexRebuilding") : t("indexRefreshing")}</span>
                    <span>{indexAction.done} / {indexAction.total}</span>
                  </div>
                  <div className="w-full h-1.5 bg-[var(--surface)] rounded-full overflow-hidden max-w-[200px]">
                    <div className="h-full bg-primary rounded-full transition-all duration-200"
                      style={{ width: indexAction.total > 0 ? `${Math.round((indexAction.done / indexAction.total) * 100)}%` : "0%" }} />
                  </div>
                </div>
              )}
              {!indexAction?.running && (
                <div className="mt-2 flex items-center gap-2">
                  <div className="w-full h-1.5 bg-[var(--surface)] rounded-full overflow-hidden max-w-[120px]">
                    <div className="h-full bg-green-600 rounded-full transition-all duration-200" style={{ width: `${indexPct}%` }} />
                  </div>
                  <span className="text-xs text-[var(--text-muted)]">
                    {indexState === "ready"
                      ? `${indexPct}% \u2022 ${indexFileCount} ${t("indexFiles")} \u2022 ${indexSymbolCount} ${t("indexSymbols")}`
                      : indexState === "building"
                        ? t("indexBuilding")
                        : t("indexIdle")}
                  </span>
                </div>
              )}
            </div>
            <div className="flex-shrink-0 flex items-center gap-2">
              <button type="button" disabled={indexAction?.running ?? false} onClick={onIndexRefresh}
                className="px-3 py-1.5 text-xs rounded bg-[var(--surface)] border border-border hover:bg-white/10 disabled:opacity-40"
                title={t("indexRefreshDesc")}>
                {t("indexSync")}
              </button>
              <button type="button" disabled={indexAction?.running ?? false} onClick={onIndexRebuild}
                className="px-3 py-1.5 text-xs rounded border border-border hover:bg-white/10 text-[var(--text-muted)] disabled:opacity-40"
                title={t("indexRebuildDesc")}>
                {t("indexDelete")}
              </button>
            </div>
          </div>
        </div>
      </section>
      )}

      {effectiveGroupId === "storage" && (
      <section id="settings-group-storage">
        <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionStorage")}</h3>
        <SettingsRow title={t("settingsAppDataRoot")} description={t("settingsAppDataRootDesc")}>
          <div className="flex items-center gap-2">
            <span className="text-xs font-mono text-[var(--text-muted)] max-w-[240px] truncate" title={appDataRoot}>{appDataRoot ?? "—"}</span>
            {appDataRoot && (
              <button type="button" onClick={() => openPath(appDataRoot)} className="px-3 py-1.5 text-xs rounded bg-[var(--surface)] border border-border hover:bg-white/10">
                {t("openFolder")}
              </button>
            )}
          </div>
        </SettingsRow>
        {onAppSettingsChange && appSettings && (
          <SettingsRow title={t("settingsAllowProjectWrites")} description={t("settingsAllowProjectWritesDesc")}>
            <div className="flex rounded-md overflow-hidden border border-border">
              <button type="button" className={`px-3 py-1.5 text-xs ${(appSettings.storage?.allowProjectWrites ?? true) !== false ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onAppSettingsChange({ ...appSettings, storage: { ...appSettings.storage, allowProjectWrites: true } })}>{t("yes")}</button>
              <button type="button" className={`px-3 py-1.5 text-xs ${(appSettings.storage?.allowProjectWrites ?? true) === false ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onAppSettingsChange({ ...appSettings, storage: { ...appSettings.storage, allowProjectWrites: false } })}>{t("no")}</button>
            </div>
          </SettingsRow>
        )}
      </section>
      )}

      {effectiveGroupId === "resources" && onAppSettingsChange && appSettings && (
      <section id="settings-group-resources">
        <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionResources")}</h3>
        <SettingsRow title={t("settingsLibrarySearchPaths")} description={t("settingsLibrarySearchPathsDesc")}>
          <input type="text" placeholder="C:\path\to\lib1;C:\path\to\lib2" value={(appSettings.resources?.librarySearchPaths ?? []).join("; ")} onChange={(e) => onAppSettingsChange({ ...appSettings, resources: { ...appSettings.resources, librarySearchPaths: e.target.value.split(";").map((s) => s.trim()).filter(Boolean) } })} className="min-w-[200px] max-w-[320px] bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded font-mono" />
        </SettingsRow>
      </section>
      )}

      {effectiveGroupId === "documentation" && onAppSettingsChange && appSettings && (
      <section id="settings-group-documentation">
        <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionDocumentation")}</h3>
        <SettingsRow title={t("settingsHelpBaseUrl")} description={t("settingsHelpBaseUrlDesc")}>
          <input type="text" placeholder="https://..." value={appSettings.documentation?.helpBaseUrl ?? ""} onChange={(e) => onAppSettingsChange({ ...appSettings, documentation: { ...appSettings.documentation, helpBaseUrl: e.target.value } })} className="min-w-[200px] max-w-[320px] bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded" />
        </SettingsRow>
        <SettingsRow title={t("settingsShowWelcome")} description={t("settingsShowWelcomeDesc")}>
          <div className="flex rounded-md overflow-hidden border border-border">
            <button type="button" className={`px-3 py-1.5 text-xs ${appSettings.documentation?.showWelcomeOnFirstLaunch !== false ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onAppSettingsChange({ ...appSettings, documentation: { ...appSettings.documentation, showWelcomeOnFirstLaunch: true } })}>{t("yes")}</button>
            <button type="button" className={`px-3 py-1.5 text-xs ${appSettings.documentation?.showWelcomeOnFirstLaunch === false ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onAppSettingsChange({ ...appSettings, documentation: { ...appSettings.documentation, showWelcomeOnFirstLaunch: false } })}>{t("no")}</button>
          </div>
        </SettingsRow>
      </section>
      )}

      {effectiveGroupId === "extensions" && onAppSettingsChange && appSettings && (
      <section id="settings-group-extensions">
        <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionExtensions")}</h3>
        <SettingsRow title={t("settingsPluginDir")} description={t("settingsPluginDirDesc")}>
          <input type="text" placeholder="" value={appSettings.extensions?.pluginDir ?? ""} onChange={(e) => onAppSettingsChange({ ...appSettings, extensions: { ...appSettings.extensions, pluginDir: e.target.value } })} className="min-w-[200px] max-w-[320px] bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded font-mono" />
        </SettingsRow>
        <SettingsRow title={t("settingsModelicaStdlibPath")} description={t("settingsModelicaStdlibPathDesc")}>
          <input type="text" placeholder="" value={appSettings.extensions?.modelicaStdlibPath ?? ""} onChange={(e) => onAppSettingsChange({ ...appSettings, extensions: { ...appSettings.extensions, modelicaStdlibPath: e.target.value } })} className="min-w-[200px] max-w-[320px] bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded font-mono" />
        </SettingsRow>
      </section>
      )}

      {effectiveGroupId === "developer" && onEnterDevMode && (
        <section id="settings-group-developer-entry">
          <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionDeveloper")}</h3>
          <div className="flex items-start justify-between gap-4 py-4">
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-1.5">
                <span className="text-sm font-medium text-[var(--text)]">{t("settingsDeveloper")}</span>
                <span className="text-[var(--text-muted)] opacity-70" title={t("devModeDesc")} aria-hidden="true">&#9432;</span>
              </div>
              <p className="text-xs text-[var(--text-muted)] mt-1">{t("devModeDesc")}</p>
            </div>
            <button type="button" className="text-xs px-3 py-1.5 bg-amber-700 hover:bg-amber-600 text-white rounded flex-shrink-0" onClick={onEnterDevMode}>{t("enterDevMode")}</button>
          </div>
        </section>
      )}
        </div>
      </main>
      </div>
    </div>
  );
}
