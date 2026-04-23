import React, { useState, useEffect, useCallback } from "react";
import { ArrowLeft } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { getApiKey, setApiKey as setApiKeyCommand, getGrokApiKey, setGrokApiKey as setGrokApiKeyCommand } from "../api/tauri";
import { t } from "../i18n";
import type { DiagramColorScheme } from "../utils/diagramColorSchemes";
import { AppIcon } from "./Icon";
import type { AppIconName } from "./Icon";
import { PREFS_KEYS, readPref } from "../utils/prefsConstants";
import { SettingsGeneralSection } from "./settings/SettingsGeneralSection";
import { SettingsAppearanceSection } from "./settings/SettingsAppearanceSection";
import { SettingsShortcutsSection } from "./settings/SettingsShortcutsSection";
import { SettingsAiModelsSection } from "./settings/SettingsAiModelsSection";
import { SettingsCompilerSection } from "./settings/SettingsCompilerSection";
import { SettingsDeveloperUsageSection } from "./settings/SettingsDeveloperUsageSection";
import { SettingsDeveloperEntrySection } from "./settings/SettingsDeveloperEntrySection";
import { SettingsCodebaseSection } from "./settings/SettingsCodebaseSection";
import { SettingsAiConfigSection } from "./settings/SettingsAiConfigSection";
import {
  SettingsAiConfigModals,
  type EditingAiItemState,
  type ViewingAiDetailState,
} from "./settings/SettingsAiConfigModals";
import { SettingsStorageSection } from "./settings/SettingsStorageSection";
import { SettingsResourcesSection } from "./settings/SettingsResourcesSection";
import { SettingsDocumentationSection } from "./settings/SettingsDocumentationSection";
import { SettingsExtensionsSection } from "./settings/SettingsExtensionsSection";
import { SettingsMslPackSection } from "./settings/SettingsMslPackSection";
import { SettingsDependencyGraphSection } from "./settings/SettingsDependencyGraphSection";
import { SettingsValidationSection } from "./settings/SettingsValidationSection";
import type {
  AppSettingsForm,
  DefaultWorkspace,
  IndexActionState,
} from "./settings/settingsTypes";

export type { AppSettingsForm, DefaultWorkspace, IndexActionState, IndexCacheSettingsForm, IndexingSettingsForm, DependencyGraphSettingsForm } from "./settings/settingsTypes";

const SETTINGS_GROUPS: { id: string; labelKey: string; icon: AppIconName; dividerAfter?: boolean }[] = [
  { id: "general", labelKey: "settingsSectionGeneral", icon: "explorer" },
  { id: "appearance", labelKey: "settingsSectionAppearance", icon: "variables" },
  { id: "shortcuts", labelKey: "settingsSectionShortcuts", icon: "columns" },
  { id: "ai-models", labelKey: "settingsSectionAiModels", icon: "ai" },
  { id: "storage", labelKey: "settingsSectionStorage", icon: "index" },
  { id: "codebase", labelKey: "settingsSectionIndexingDocs", icon: "index" },
  { id: "ai-config", labelKey: "settingsSectionAiConfig", icon: "ai" },
  { id: "resources", labelKey: "settingsSectionResources", icon: "library" },
  { id: "documentation", labelKey: "settingsSectionDocumentation", icon: "columns" },
  { id: "extensions", labelKey: "settingsSectionExtensions", icon: "iterate" },
  { id: "validation", labelKey: "settingsSectionValidation", icon: "simSettings" },
  { id: "dependency-graph", labelKey: "settingsSectionDependencyGraph", icon: "simSettings" },
  { id: "compiler", labelKey: "settingsSectionJitCompiler", icon: "simSettings" },
  { id: "developer", labelKey: "settingsSectionDeveloper", icon: "ai", dividerAfter: true },
];

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
  projectDir?: string | null;
  initialGroupId?: string | null;
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
  projectDir,
  initialGroupId,
}: SettingsContentProps) {
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [includedFilesResult, setIncludedFilesResult] = useState<{ total: number; paths: string[] } | null>(null);
  const [apiKeySaved, setApiKeySaved] = useState(false);
  const [apiKeyBanner, setApiKeyBanner] = useState<string | null>(null);
  const [grokApiKeyInput, setGrokApiKeyInput] = useState("");
  const [grokApiKeySaved, setGrokApiKeySaved] = useState(false);
  const [grokApiKeyBanner, setGrokApiKeyBanner] = useState<string | null>(null);

  const [compilerExe, setCompilerExe] = useState("");
  const [compilerArgs, setCompilerArgs] = useState("");
  const [compilerConfigBanner, setCompilerConfigBanner] = useState<string | null>(null);
  const [editingAiItem, setEditingAiItem] = useState<EditingAiItemState>(null);
  const [editingAiError, setEditingAiError] = useState<string | null>(null);
  const [viewingAiDetail, setViewingAiDetail] = useState<ViewingAiDetailState>(null);

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
  const [regressionAutoLoadOnOpen, setRegressionAutoLoadOnOpen] = useState<boolean>(() =>
    readPref(PREFS_KEYS.regressionAutoLoadOnOpen, (s) => s !== "false", true)
  );
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
    getGrokApiKey()
      .then(() => setGrokApiKeySaved(true))
      .catch(() => setGrokApiKeySaved(false));
  }, []);

  useEffect(() => {
    if (apiKeyBanner) {
      const tm = setTimeout(() => setApiKeyBanner(null), 3000);
      return () => clearTimeout(tm);
    }
  }, [apiKeyBanner]);

  useEffect(() => {
    if (grokApiKeyBanner) {
      const tm = setTimeout(() => setGrokApiKeyBanner(null), 3000);
      return () => clearTimeout(tm);
    }
  }, [grokApiKeyBanner]);

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

  const handleSaveGrokApiKey = useCallback(async () => {
    if (!grokApiKeyInput.trim()) return;
    try {
      await setGrokApiKeyCommand(grokApiKeyInput.trim());
      setGrokApiKeySaved(true);
      setGrokApiKeyInput("");
      setGrokApiKeyBanner(t("apiKeySaveSuccess"));
    } catch (e) {
      setGrokApiKeyBanner(String(e));
    }
  }, [grokApiKeyInput]);

  const handleClearGrokApiKey = useCallback(async () => {
    try {
      await setGrokApiKeyCommand("");
      setGrokApiKeySaved(false);
      setGrokApiKeyBanner(t("apiKeyClearSuccess"));
    } catch (e) {
      setGrokApiKeyBanner(String(e));
    }
  }, []);

  const indexPct = indexState === "ready" && indexFileCount > 0 ? 100 : (indexAction?.total ? Math.round((indexAction.done / indexAction.total) * 100) : 0);

  const showDeveloperGroup = Boolean(aiDailyLimit != null);
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

  useEffect(() => {
    if (initialGroupId && visibleGroups.some((g) => g.id === initialGroupId))
      setActiveGroupId(initialGroupId);
  }, [initialGroupId, visibleGroups]);

  const selectGroup = useCallback((id: string) => setActiveGroupId(id), []);

  return (
    <div className="flex flex-1 min-h-0 flex-col text-[var(--text)]">
      {onClose && (
        <div className="panel-header-bar shrink-0 flex items-center border-b border-[var(--border)]">
          <button
            type="button"
            onClick={onClose}
            className="p-2 rounded text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-[var(--surface-hover)]"
            title={t("closeSettings")}
            aria-label={t("closeSettings")}
          >
            <ArrowLeft size={18} />
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
        <SettingsGeneralSection
          defaultWorkspace={defaultWorkspace}
          onDefaultWorkspaceChange={onDefaultWorkspaceChange}
          restoreLayout={restoreLayout}
          onRestoreLayoutChange={onRestoreLayoutChange}
          regressionAutoLoadOnOpen={regressionAutoLoadOnOpen}
          onRegressionAutoLoadOnOpenChange={setRegressionAutoLoadOnOpen}
          lang={lang}
          onLangChange={onLangChange}
        />
      )}

      {effectiveGroupId === "ai-models" && (
        <SettingsAiModelsSection
          apiKeyInput={apiKeyInput}
          setApiKeyInput={setApiKeyInput}
          apiKeySaved={apiKeySaved}
          apiKeyBanner={apiKeyBanner}
          grokApiKeyInput={grokApiKeyInput}
          setGrokApiKeyInput={setGrokApiKeyInput}
          grokApiKeySaved={grokApiKeySaved}
          grokApiKeyBanner={grokApiKeyBanner}
          onSaveApiKey={handleSaveApiKey}
          onClearApiKey={handleClearApiKey}
          onSaveGrokApiKey={handleSaveGrokApiKey}
          onClearGrokApiKey={handleClearGrokApiKey}
          appSettings={appSettings}
          onAppSettingsChange={onAppSettingsChange}
          aiModel={aiModel}
          onAiModelChange={onAiModelChange}
        />
      )}

      {effectiveGroupId === "validation" && onAppSettingsChange && appSettings && (
        <SettingsValidationSection appSettings={appSettings} onAppSettingsChange={onAppSettingsChange} />
      )}

      {effectiveGroupId === "dependency-graph" && onAppSettingsChange && appSettings && (
        <SettingsDependencyGraphSection appSettings={appSettings} onAppSettingsChange={onAppSettingsChange} />
      )}

      {effectiveGroupId === "compiler" && (
        <SettingsCompilerSection
          compilerExe={compilerExe}
          setCompilerExe={setCompilerExe}
          compilerArgs={compilerArgs}
          setCompilerArgs={setCompilerArgs}
          compilerConfigBanner={compilerConfigBanner}
          onSaveCompilerConfig={handleSaveCompilerConfig}
        />
      )}

      {effectiveGroupId === "developer" && aiDailyLimit != null && (
        <SettingsDeveloperUsageSection
          aiDailyUsed={aiDailyUsed}
          aiDailyLimit={aiDailyLimit}
          onAiDailyReset={onAiDailyReset}
        />
      )}

      {effectiveGroupId === "appearance" && (
        <SettingsAppearanceSection
          theme={theme}
          onThemeChange={onThemeChange}
          fontUi={fontUi}
          onFontUiChange={handleFontUiChange}
          fontSizePercent={fontSizePercent}
          onFontSizePercentChange={handleFontSizePercentChange}
          uiColorScheme={uiColorScheme}
          onUiColorSchemeChange={onUiColorSchemeChange}
          panelHeaderHeight={panelHeaderHeight}
          onPanelHeaderHeightChange={onPanelHeaderHeightChange}
          toolbarBtnSize={toolbarBtnSize}
          onToolbarBtnSizeChange={onToolbarBtnSizeChange}
          toolbarGap={toolbarGap}
          onToolbarGapChange={onToolbarGapChange}
          diagramSchemeId={diagramSchemeId}
          diagramScheme={diagramScheme}
          onDiagramSchemeChange={onDiagramSchemeChange}
        />
      )}

      {effectiveGroupId === "shortcuts" && <SettingsShortcutsSection />}

      {effectiveGroupId === "codebase" && (
        <SettingsCodebaseSection
          indexFileCount={indexFileCount}
          indexSymbolCount={indexSymbolCount}
          indexState={indexState}
          indexAction={indexAction}
          onIndexRefresh={onIndexRefresh}
          onIndexRebuild={onIndexRebuild}
          indexPct={indexPct}
          projectDir={projectDir}
          appSettings={appSettings}
          onAppSettingsChange={onAppSettingsChange}
          includedFilesResult={includedFilesResult}
          setIncludedFilesResult={setIncludedFilesResult}
        />
      )}

      <SettingsAiConfigModals
        editingAiItem={editingAiItem}
        setEditingAiItem={setEditingAiItem}
        editingAiError={editingAiError}
        setEditingAiError={setEditingAiError}
        viewingAiDetail={viewingAiDetail}
        setViewingAiDetail={setViewingAiDetail}
        appSettings={appSettings}
        onAppSettingsChange={onAppSettingsChange}
      />

      {effectiveGroupId === "ai-config" && onAppSettingsChange && appSettings && (
        <SettingsAiConfigSection
          appSettings={appSettings}
          onAppSettingsChange={onAppSettingsChange}
          setEditingAiItem={setEditingAiItem}
          setViewingAiDetail={setViewingAiDetail}
        />
      )}

      {effectiveGroupId === "storage" && (
        <SettingsStorageSection appDataRoot={appDataRoot} appSettings={appSettings} onAppSettingsChange={onAppSettingsChange} />
      )}

      {effectiveGroupId === "resources" && onAppSettingsChange && appSettings && (
        <SettingsResourcesSection appSettings={appSettings} onAppSettingsChange={onAppSettingsChange} />
      )}

      {effectiveGroupId === "documentation" && onAppSettingsChange && appSettings && (
        <SettingsDocumentationSection appSettings={appSettings} onAppSettingsChange={onAppSettingsChange} />
      )}

      {effectiveGroupId === "extensions" && onAppSettingsChange && appSettings && (
        <>
          <SettingsExtensionsSection appSettings={appSettings} onAppSettingsChange={onAppSettingsChange} />
          <SettingsMslPackSection appSettings={appSettings} onAppSettingsChange={onAppSettingsChange} />
        </>
      )}

      {effectiveGroupId === "developer" && onEnterDevMode && (
        <SettingsDeveloperEntrySection onEnterDevMode={onEnterDevMode} />
      )}

        </div>
      </main>
      </div>
    </div>
  );
}
