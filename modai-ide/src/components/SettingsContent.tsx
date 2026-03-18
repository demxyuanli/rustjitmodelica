import React, { useState, useEffect, useCallback } from "react";
import { Pencil, X, Check, FolderOpen, FileText, RefreshCw, RotateCw, Trash2, ArrowLeft } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { openPath } from "@tauri-apps/plugin-opener";
import { getApiKey, setApiKey as setApiKeyCommand, getGrokApiKey, setGrokApiKey as setGrokApiKeyCommand, rebuildComponentLibraryIndex, indexListIncludedFiles, type AiConfig, type AiRule, type AiSkill, type AiSubagent, type AiCommand } from "../api/tauri";
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
import { BUILTIN_AI_MODELS, filterEnabledModels } from "../constants/aiModels";

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
  { id: "compiler", labelKey: "settingsSectionJitCompiler", icon: "simSettings" },
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

export interface IndexCacheSettingsForm {
  componentLibraryIndexEnabled?: boolean;
  repoIndexRefreshOnJitLoad?: boolean;
  gitStatusThrottleMs?: number;
}

export interface IndexingSettingsForm {
  indexAutoNewFolders?: boolean;
  indexAutoNewFoldersMaxFiles?: number;
  indexRepoForGrep?: boolean;
}

export interface AppSettingsForm {
  storage?: { indexPathPolicy?: string; allowProjectWrites?: boolean };
  resources?: { librarySearchPaths?: string[]; packageCacheDir?: string };
  documentation?: { helpBaseUrl?: string; showWelcomeOnFirstLaunch?: boolean };
  extensions?: { pluginDir?: string; modelicaStdlibPath?: string };
  indexCache?: IndexCacheSettingsForm;
  indexing?: IndexingSettingsForm;
  ai?: AiConfig;
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
  projectDir?: string | null;
  initialGroupId?: string | null;
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

function SettingsSwitch({
  checked,
  onChange,
  ariaLabel,
}: { checked: boolean; onChange: (v: boolean) => void; ariaLabel: string }) {
  return (
    <label className="flex items-center cursor-pointer select-none">
      <span className={`relative inline-flex h-5 w-9 shrink-0 rounded-full transition-colors ${checked ? "bg-primary" : "bg-[var(--surface-hover)]"}`}>
        <input
          type="checkbox"
          checked={checked}
          onChange={(e) => onChange(e.target.checked)}
          className="sr-only"
          aria-label={ariaLabel}
        />
        <span className={`pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow-sm transition-transform mt-0.5 ml-0.5 ${checked ? "translate-x-4" : "translate-x-0"}`} />
      </span>
    </label>
  );
}

function RebuildComponentLibraryIndexButton() {
  const [status, setStatus] = useState<"idle" | "running" | "done">("idle");
  const handleClick = useCallback(async () => {
    setStatus("running");
    try {
      await rebuildComponentLibraryIndex();
      setStatus("done");
      const t = setTimeout(() => setStatus("idle"), 2000);
      return () => clearTimeout(t);
    } catch {
      setStatus("idle");
    }
  }, []);
  return (
    <button
      type="button"
      onClick={handleClick}
      disabled={status === "running"}
      className="p-2 rounded-md bg-[var(--surface)] border border-border hover:bg-white/10 disabled:opacity-40 text-[var(--text-muted)]"
      title={status === "running" ? t("indexRefreshing") : status === "done" ? t("saved") : t("indexRebuild")}
      aria-label={status === "running" ? t("indexRefreshing") : status === "done" ? t("saved") : t("indexRebuild")}
    >
      <RotateCw size={16} className={status === "running" ? "animate-spin" : undefined} />
    </button>
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
  const [editingAiItem, setEditingAiItem] = useState<
    | {
        kind: "rule";
        item: AiRule;
      }
    | {
        kind: "skill";
        item: AiSkill;
      }
    | {
        kind: "subagent";
        item: AiSubagent;
      }
    | {
        kind: "command";
        item: AiCommand;
      }
    | null
  >(null);
  const [editingAiError, setEditingAiError] = useState<string | null>(null);
  const [viewingAiDetail, setViewingAiDetail] = useState<
    | { kind: "rule"; item: AiRule }
    | { kind: "skill"; item: AiSkill }
    | { kind: "subagent"; item: AiSubagent }
    | { kind: "command"; item: AiCommand }
    | null
  >(null);

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
          <SettingsSwitch checked={restoreLayout} onChange={(v) => onRestoreLayoutChange?.(v)} ariaLabel={t("settingsRestoreLayout")} />
        </SettingsRow>
        <SettingsRow title={t("settingsLanguage")} description={t("settingsLanguageDesc")}>
          <div className="flex rounded-md overflow-hidden border border-border">
            <button type="button" className={`px-3 py-1.5 text-xs ${lang === "zh" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onLangChange?.("zh")}>{t("switchLanguageToChinese")}</button>
            <button type="button" className={`px-3 py-1.5 text-xs ${lang === "en" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onLangChange?.("en")}>{t("switchLanguageToEnglish")}</button>
          </div>
        </SettingsRow>
      </section>
      )}

      {effectiveGroupId === "ai-models" && (
      <section id="settings-group-ai-models">
        <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">{t("settingsSectionAiModels")}</h3>
        <div className="space-y-8">
          <div className="rounded-lg bg-[var(--surface-muted)]/20 p-4">
            <h4 className="text-sm font-semibold text-[var(--text)] mb-1">{t("settingsApiKey")}</h4>
            <p className="text-xs text-[var(--text-muted)] mb-4">{t("settingsApiKeyDesc")}</p>
            {apiKeyBanner && (
              <div className="mb-3 px-3 py-2 rounded text-xs bg-green-900/30 text-green-300 border border-green-700">{apiKeyBanner}</div>
            )}
            {apiKeySaved ? (
              <div className="flex items-center gap-2">
                <span className="text-xs text-green-400">{t("apiKeySaved")}</span>
                <button type="button" onClick={handleClearApiKey}
                  className="p-2 rounded-md bg-[var(--surface)] border border-border hover:bg-white/10 text-[var(--text-muted)]"
                  title={t("apiKeyClear")}
                  aria-label={t("apiKeyClear")}>
                  <Trash2 size={16} />
                </button>
              </div>
            ) : (
              <div className="flex gap-2 items-center flex-wrap">
                <input type="password" placeholder={t("apiKeyPlaceholder")} value={apiKeyInput}
                  onChange={(e) => setApiKeyInput(e.target.value)}
                  onKeyDown={(e) => { if (e.key === "Enter") handleSaveApiKey(); }}
                  className="w-48 bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded" />
                <button type="button" onClick={handleSaveApiKey} disabled={!apiKeyInput.trim()}
                  className="p-2 rounded-md bg-primary hover:bg-blue-600 text-white disabled:opacity-40"
                  title={t("save")}
                  aria-label={t("save")}>
                  <Check size={16} />
                </button>
              </div>
            )}
          </div>

          <div className="rounded-lg bg-[var(--surface-muted)]/20 p-4">
            <h4 className="text-sm font-semibold text-[var(--text)] mb-1">{t("settingsGrokApiKey")}</h4>
            <p className="text-xs text-[var(--text-muted)] mb-4">{t("settingsGrokApiKeyDesc")}</p>
            {grokApiKeyBanner && (
              <div className="mb-3 px-3 py-2 rounded text-xs bg-green-900/30 text-green-300 border border-green-700">{grokApiKeyBanner}</div>
            )}
            {grokApiKeySaved ? (
              <div className="flex items-center gap-2">
                <span className="text-xs text-green-400">{t("apiKeySaved")}</span>
                <button type="button" onClick={handleClearGrokApiKey}
                  className="p-2 rounded-md bg-[var(--surface)] border border-border hover:bg-white/10 text-[var(--text-muted)]"
                  title={t("apiKeyClear")}
                  aria-label={t("apiKeyClear")}>
                  <Trash2 size={16} />
                </button>
              </div>
            ) : (
              <div className="flex gap-2 items-center flex-wrap">
                <input type="password" placeholder={t("apiKeyPlaceholder")} value={grokApiKeyInput}
                  onChange={(e) => setGrokApiKeyInput(e.target.value)}
                  onKeyDown={(e) => { if (e.key === "Enter") handleSaveGrokApiKey(); }}
                  className="w-48 bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded" />
                <button type="button" onClick={handleSaveGrokApiKey} disabled={!grokApiKeyInput.trim()}
                  className="p-2 rounded-md bg-primary hover:bg-blue-600 text-white disabled:opacity-40"
                  title={t("save")}
                  aria-label={t("save")}>
                  <Check size={16} />
                </button>
              </div>
            )}
          </div>

          {onAppSettingsChange && appSettings && (
          <div className="rounded-lg bg-[var(--surface-muted)]/20 p-4">
            <h4 className="text-sm font-semibold text-[var(--text)] mb-1">{t("settingsAiModelsAvailable")}</h4>
            <p className="text-xs text-[var(--text-muted)] mb-4">{t("settingsAiModelsAvailableDesc")}</p>
            <div className="space-y-0">
              {BUILTIN_AI_MODELS.map((m) => {
                const enabledList = appSettings?.ai?.modelIdsEnabled;
                const isEnabled = !enabledList || enabledList.length === 0 || enabledList.includes(m.id);
                return (
                  <div key={m.id} className="flex items-center justify-between py-3 border-b border-[var(--border)] last:border-b-0">
                    <div>
                      <span className="text-sm text-[var(--text)]">{m.label}</span>
                      <span className="ml-2 text-[10px] uppercase text-[var(--text-muted)]">
                        {m.provider === "ollama" ? t("aiModelProviderOllama") : m.provider === "grok" ? t("aiModelProviderGrok") : t("aiModelProviderDeepSeek")}
                      </span>
                    </div>
                    <SettingsSwitch
                      checked={isEnabled}
                      onChange={(checked) => {
                        const next = enabledList && enabledList.length > 0 ? [...enabledList] : BUILTIN_AI_MODELS.map((x) => x.id);
                        const nextSet = new Set(next);
                        if (checked) nextSet.add(m.id);
                        else nextSet.delete(m.id);
                        const nextList = Array.from(nextSet);
                        onAppSettingsChange({
                          ...appSettings,
                          ai: {
                            ...(appSettings.ai ?? { rules: [], skills: [], subagents: [], commands: [] }),
                            modelIdsEnabled: nextList.length === BUILTIN_AI_MODELS.length ? undefined : nextList,
                          },
                        });
                      }}
                      ariaLabel={m.label}
                    />
                  </div>
                );
              })}
            </div>
          </div>
          )}

          {onAiModelChange != null && onAppSettingsChange && appSettings && (
          <div className="rounded-lg bg-[var(--surface-muted)]/20 p-4">
            <h4 className="text-sm font-semibold text-[var(--text)] mb-1">{t("settingsAiModelDefault")}</h4>
            <p className="text-xs text-[var(--text-muted)] mb-4">{t("settingsAiModelManageDesc")}</p>
            <select
              value={aiModel || "deepseek-chat"}
              onChange={(e) => onAiModelChange(e.target.value)}
              className="bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded text-[var(--text)] min-w-[200px]"
              aria-label={t("settingsAiModel")}
            >
              {(() => {
                const enabled = filterEnabledModels(appSettings?.ai?.modelIdsEnabled);
                const deepseek = enabled.filter((m) => m.provider === "deepseek");
                const grok = enabled.filter((m) => m.provider === "grok");
                const ollama = enabled.filter((m) => m.provider === "ollama");
                return (
                  <>
                    {deepseek.length > 0 && (
                      <optgroup label={t("aiModelProviderDeepSeek")}>
                        {deepseek.map((m) => (
                          <option key={m.id} value={m.id}>{m.label}</option>
                        ))}
                      </optgroup>
                    )}
                    {grok.length > 0 && (
                      <optgroup label={t("aiModelProviderGrok")}>
                        {grok.map((m) => (
                          <option key={m.id} value={m.id}>{m.label}</option>
                        ))}
                      </optgroup>
                    )}
                    {ollama.length > 0 && (
                      <optgroup label={t("aiModelProviderOllama")}>
                        {ollama.map((m) => (
                          <option key={m.id} value={m.id}>{m.label}</option>
                        ))}
                      </optgroup>
                    )}
                  </>
                );
              })()}
            </select>
          </div>
          )}
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
              <button type="button" onClick={handleSaveCompilerConfig}
                className="p-2 rounded-md bg-primary hover:bg-blue-600 text-white"
                title={t("save")}
                aria-label={t("save")}>
                <Check size={16} />
              </button>
            </div>
          </SettingsRow>
        </div>
      </section>
      )}

      {effectiveGroupId === "developer" && aiDailyLimit != null && (
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
        <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionIndexingDocs")}</h3>
        <div className="space-y-6">
          <div>
            <h4 className="text-xs font-semibold text-[var(--text-muted)] uppercase tracking-wider mb-2">{t("settingsSubsectionIndexOverview")}</h4>
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
                className="p-2 rounded-md bg-[var(--surface)] border border-border hover:bg-white/10 disabled:opacity-40 text-[var(--text-muted)]"
                title={t("indexRefreshDesc")}
                aria-label={t("indexSync")}>
                <RefreshCw size={16} className={indexAction?.running ? "animate-spin" : undefined} />
              </button>
              <button type="button" disabled={indexAction?.running ?? false} onClick={onIndexRebuild}
                className="p-2 rounded-md border border-border hover:bg-white/10 text-[var(--text-muted)] disabled:opacity-40"
                title={t("indexRebuildDesc")}
                aria-label={t("indexDelete")}>
                <Trash2 size={16} />
              </button>
            </div>
          </div>
          <p className="text-xs text-[var(--text-muted)] mt-1 mb-2">{t("settingsAllDataStoredLocally")}</p>
            </div>
          </div>
          {onAppSettingsChange && appSettings && (
            <>
            <div>
              <h4 className="text-xs font-semibold text-[var(--text-muted)] uppercase tracking-wider mb-2">{t("settingsSubsectionIndexingBehavior")}</h4>
              <SettingsRow title={t("settingsIndexNewFolders")} description={t("settingsIndexNewFoldersDesc")}>
                <div className="flex items-center gap-2">
                  <SettingsSwitch checked={(appSettings.indexing?.indexAutoNewFolders ?? true) !== false} onChange={(v) => onAppSettingsChange({ ...appSettings, indexing: { ...appSettings.indexing, indexAutoNewFolders: v } })} ariaLabel={t("settingsIndexNewFolders")} />
                  <span className="text-xs text-[var(--text-muted)]">&lt; {appSettings.indexing?.indexAutoNewFoldersMaxFiles ?? 50000} {t("indexFiles")}</span>
                </div>
              </SettingsRow>
            </div>
            <div>
              <h4 className="text-xs font-semibold text-[var(--text-muted)] uppercase tracking-wider mb-2">{t("settingsSubsectionIgnoreRules")}</h4>
              <SettingsRow title={t("settingsIgnoreFiles")} description={t("settingsIgnoreFilesDesc")}>
                <div className="flex items-center gap-2">
                  <button type="button" onClick={() => projectDir && openPath(projectDir)} disabled={!projectDir} className="p-2 rounded-md bg-[var(--surface)] border border-border hover:bg-white/10 disabled:opacity-40 text-[var(--text-muted)]" title={t("openFolder")} aria-label={t("openFolder")}>
                    <FolderOpen size={16} />
                  </button>
                  <button
                    type="button"
                    disabled={!projectDir}
                    className="p-2 rounded-md bg-[var(--surface)] border border-border hover:bg-white/10 disabled:opacity-40 text-[var(--text-muted)]"
                    title={t("settingsViewIncludedFiles")}
                    aria-label={t("settingsViewIncludedFiles")}
                    onClick={async () => {
                      if (!projectDir) return;
                      try {
                        const r = await indexListIncludedFiles(projectDir, 500);
                        setIncludedFilesResult({ total: r.total, paths: r.paths });
                      } catch {
                        setIncludedFilesResult({ total: 0, paths: [] });
                      }
                    }}
                  >
                    <FileText size={16} />
                  </button>
                </div>
              </SettingsRow>
            </div>
            <div>
              <h4 className="text-xs font-semibold text-[var(--text-muted)] uppercase tracking-wider mb-2">{t("settingsSubsectionRepoAndComponent")}</h4>
              <SettingsRow title={t("settingsIndexRepoForGrep")} description={t("settingsIndexRepoForGrepDesc")}>
                <SettingsSwitch checked={(appSettings.indexing?.indexRepoForGrep ?? true) !== false} onChange={(v) => onAppSettingsChange({ ...appSettings, indexing: { ...appSettings.indexing, indexRepoForGrep: v } })} ariaLabel={t("settingsIndexRepoForGrep")} />
              </SettingsRow>
              <SettingsRow title={t("settingsComponentLibraryIndex")} description={t("settingsComponentLibraryIndexDesc")}>
                <SettingsSwitch checked={(appSettings.indexCache?.componentLibraryIndexEnabled ?? true) !== false} onChange={(v) => onAppSettingsChange({ ...appSettings, indexCache: { ...appSettings.indexCache, componentLibraryIndexEnabled: v } })} ariaLabel={t("settingsComponentLibraryIndex")} />
              </SettingsRow>
              <SettingsRow title={t("settingsRepoIndexRefreshOnJitLoad")} description={t("settingsRepoIndexRefreshOnJitLoadDesc")}>
                <SettingsSwitch checked={(appSettings.indexCache?.repoIndexRefreshOnJitLoad ?? true) !== false} onChange={(v) => onAppSettingsChange({ ...appSettings, indexCache: { ...appSettings.indexCache, repoIndexRefreshOnJitLoad: v } })} ariaLabel={t("settingsRepoIndexRefreshOnJitLoad")} />
              </SettingsRow>
              <SettingsRow title={t("settingsGitStatusThrottleMs")} description={t("settingsGitStatusThrottleMsDesc")}>
                <input type="number" min={500} max={60000} step={500} value={appSettings.indexCache?.gitStatusThrottleMs ?? 2000} onChange={(e) => { const v = parseInt(e.target.value, 10); if (!Number.isNaN(v)) onAppSettingsChange({ ...appSettings, indexCache: { ...appSettings.indexCache, gitStatusThrottleMs: Math.max(500, Math.min(60000, v)) } }); }} className="w-24 bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded font-mono" />
                <span className="text-xs text-[var(--text-muted)]"> ms</span>
              </SettingsRow>
              <SettingsRow title={t("indexRebuildComponentLibrary")} description={t("indexRebuildComponentLibraryDesc")}>
                <RebuildComponentLibraryIndexButton />
              </SettingsRow>
            </div>
            </>
          )}
        </div>
        {includedFilesResult !== null && (
          <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50" role="dialog" aria-modal="true" onClick={() => setIncludedFilesResult(null)}>
            <div className="bg-[var(--surface)] border border-border rounded-lg shadow-xl max-w-2xl w-full max-h-[80vh] flex flex-col m-4" onClick={(e) => e.stopPropagation()}>
              <div className="flex items-center justify-between p-3 border-b border-border">
                <span className="text-sm font-medium">{t("settingsViewIncludedFiles")}</span>
                <button type="button" onClick={() => setIncludedFilesResult(null)} className="p-1.5 rounded hover:bg-white/10 text-[var(--text-muted)]" aria-label={t("cancel")}><X size={16} /></button>
              </div>
              <div className="p-3 text-xs text-[var(--text-muted)]">
                {includedFilesResult.total} {t("indexFiles")}{includedFilesResult.paths.length < includedFilesResult.total ? ` (first ${includedFilesResult.paths.length})` : ""}
              </div>
              <div className="flex-1 min-h-0 overflow-auto p-3 pt-0 font-mono text-xs text-[var(--text)] break-all">
                {includedFilesResult.paths.length === 0 ? (
                  <span className="text-[var(--text-muted)]">—</span>
                ) : (
                  <ul className="list-none space-y-0.5">
                    {includedFilesResult.paths.map((p, i) => (
                      <li key={i}>{p}</li>
                    ))}
                  </ul>
                )}
              </div>
            </div>
          </div>
        )}
      </section>
      )}

      {editingAiItem && onAppSettingsChange && appSettings && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50" role="dialog" aria-modal="true" onClick={() => setEditingAiItem(null)}>
          <div className="bg-[var(--surface)] border border-border rounded-lg shadow-xl max-w-2xl w-full max-h-[80vh] flex flex-col m-4" onClick={(e) => e.stopPropagation()}>
            <div className="flex items-center justify-between p-3 border-b border-border">
              <span className="text-sm font-medium text-[var(--text)]">
                {editingAiItem.kind === "rule" && t("settingsAiRules")}
                {editingAiItem.kind === "skill" && t("settingsAiSkills")}
                {editingAiItem.kind === "subagent" && t("settingsAiSubagents")}
                {editingAiItem.kind === "command" && t("settingsAiCommands")}
              </span>
              <button type="button" onClick={() => setEditingAiItem(null)} className="p-1.5 rounded hover:bg-white/10 text-[var(--text-muted)]" aria-label={t("cancel")}><X size={16} /></button>
            </div>
            <div className="p-3 space-y-3 text-xs">
              {editingAiError && (
                <div className="px-2 py-1 rounded bg-red-900/40 border border-red-700 text-[11px] text-red-200">
                  {editingAiError}
                </div>
              )}
              <div className="flex flex-col gap-1">
                <label className="text-[var(--text-muted)]">{t("settingsAiEditName")}</label>
                <input
                  className="bg-[var(--surface)] border border-border rounded px-2 py-1 text-xs text-[var(--text)]"
                  value={editingAiItem.item.name}
                  onChange={(e) => {
                    setEditingAiError(null);
                    setEditingAiItem({ ...editingAiItem, item: { ...editingAiItem.item, name: e.target.value } as any });
                  }}
                />
              </div>
              {"description" in editingAiItem.item && (
                <div className="flex flex-col gap-1">
                  <label className="text-[var(--text-muted)]">{t("settingsAiEditDescription")}</label>
                  <input
                    className="bg-[var(--surface)] border border-border rounded px-2 py-1 text-xs text-[var(--text)]"
                    value={(editingAiItem.item as any).description ?? ""}
                    onChange={(e) => {
                      setEditingAiError(null);
                      setEditingAiItem({ ...editingAiItem, item: { ...(editingAiItem.item as any), description: e.target.value } });
                    }}
                  />
                </div>
              )}
              <div className="flex flex-col gap-1">
                <label className="text-[var(--text-muted)]">{t("settingsAiEditScope")}</label>
                <select
                  className="bg-[var(--surface)] border border-border rounded px-2 py-1 text-xs text-[var(--text)]"
                  value={editingAiItem.item.scope}
                  onChange={(e) => {
                    setEditingAiError(null);
                    setEditingAiItem({ ...editingAiItem, item: { ...editingAiItem.item, scope: e.target.value as any } as any });
                  }}
                  disabled={editingAiItem.item.scope === "rustmodlica"}
                >
                  <option value="user">user</option>
                  <option value="project">project</option>
                  <option value="rustmodlica">rustmodlica</option>
                  <option value="all">all</option>
                </select>
              </div>
              <div className="flex flex-col gap-1">
                <label className="text-[var(--text-muted)]">{t("settingsAiEditContent")}</label>
                <textarea
                  className="bg-[var(--surface)] border border-border rounded px-2 py-1 text-xs text-[var(--text)] h-40 resize-none"
                  value={editingAiItem.item.content}
                  onChange={(e) => {
                    setEditingAiError(null);
                    setEditingAiItem({ ...editingAiItem, item: { ...editingAiItem.item, content: e.target.value } as any });
                  }}
                />
              </div>
            </div>
            <div className="flex items-center justify-end gap-2 p-3 border-t border-border">
              <button
                type="button"
                className="p-2 rounded-md bg-[var(--surface)] border border-border hover:bg-white/10 text-[var(--text-muted)]"
                title={t("cancel")}
                aria-label={t("cancel")}
                onClick={() => {
                  setEditingAiError(null);
                  setEditingAiItem(null);
                }}
              >
                <X size={16} />
              </button>
              <button
                type="button"
                className="p-2 rounded-md bg-primary text-white"
                title={t("save")}
                aria-label={t("save")}
                onClick={() => {
                  const trimmedName = editingAiItem.item.name.trim();
                  const trimmedContent = editingAiItem.item.content.trim();
                  if (!trimmedName || !trimmedContent) {
                    setEditingAiError(t("settingsAiValidationEmpty"));
                    return;
                  }
                  const ai = appSettings.ai ?? { rules: [], skills: [], subagents: [], commands: [] };
                  if (editingAiItem.kind === "rule") {
                    const list = [...ai.rules];
                    const idx = list.findIndex((x) => x.id === editingAiItem.item.id);
                    if (idx >= 0) list[idx] = editingAiItem.item as AiRule;
                    onAppSettingsChange({ ...appSettings, ai: { ...ai, rules: list } });
                  } else if (editingAiItem.kind === "skill") {
                    const list = [...ai.skills];
                    const idx = list.findIndex((x) => x.id === editingAiItem.item.id);
                    if (idx >= 0) list[idx] = editingAiItem.item as AiSkill;
                    onAppSettingsChange({ ...appSettings, ai: { ...ai, skills: list } });
                  } else if (editingAiItem.kind === "subagent") {
                    const list = [...ai.subagents];
                    const idx = list.findIndex((x) => x.id === editingAiItem.item.id);
                    if (idx >= 0) list[idx] = editingAiItem.item as AiSubagent;
                    onAppSettingsChange({ ...appSettings, ai: { ...ai, subagents: list } });
                  } else if (editingAiItem.kind === "command") {
                    const list = [...ai.commands];
                    const idx = list.findIndex((x) => x.id === editingAiItem.item.id);
                    if (idx >= 0) list[idx] = editingAiItem.item as AiCommand;
                    onAppSettingsChange({ ...appSettings, ai: { ...ai, commands: list } });
                  }
                  setEditingAiError(null);
                  setEditingAiItem(null);
                }}
              >
                <Check size={16} />
              </button>
            </div>
          </div>
        </div>
      )}

      {viewingAiDetail && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50" role="dialog" aria-modal="true" onClick={() => setViewingAiDetail(null)}>
          <div className="bg-[var(--surface)] border border-[var(--border)] rounded-lg shadow-xl max-w-2xl w-full max-h-[85vh] flex flex-col m-4" onClick={(e) => e.stopPropagation()}>
            <div className="flex items-center justify-between p-3 border-b border-[var(--border)] flex-shrink-0">
              <span className="text-sm font-medium text-[var(--text)] truncate pr-2">
                {viewingAiDetail.kind === "rule" && t("settingsSubsectionAiRules")}
                {viewingAiDetail.kind === "skill" && t("settingsSubsectionAiSkills")}
                {viewingAiDetail.kind === "subagent" && t("settingsSubsectionAiSubagents")}
                {viewingAiDetail.kind === "command" && t("settingsSubsectionAiCommands")}
                {" — "}
                {viewingAiDetail.item.name}
              </span>
              <button type="button" onClick={() => setViewingAiDetail(null)} className="p-1.5 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)]" aria-label={t("cancel")}>
                <X size={16} />
              </button>
            </div>
            <div className="p-3 overflow-y-auto min-h-0 flex-1" style={{ maxHeight: "60vh" }}>
              {"description" in viewingAiDetail.item && (viewingAiDetail.item as any).description && (
                <p className="text-xs text-[var(--text-muted)] mb-2">{(viewingAiDetail.item as any).description}</p>
              )}
              <pre className="text-xs text-[var(--text)] whitespace-pre-wrap break-words font-sans">{viewingAiDetail.item.content ?? ""}</pre>
            </div>
          </div>
        </div>
      )}

      {effectiveGroupId === "ai-config" && onAppSettingsChange && appSettings && (
      <section id="settings-group-ai-config">
        <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">{t("settingsSectionAiConfig")}</h3>
        <div className="space-y-8">
          <div className="rounded-lg bg-[var(--surface-muted)]/20 p-4">
            <h4 className="text-sm font-semibold text-[var(--text)] mb-1">{t("settingsSubsectionAiRules")}</h4>
            <p className="text-xs text-[var(--text-muted)] mb-4">{t("settingsAiRulesDesc")}</p>
            <div className="space-y-0">
              {(appSettings.ai?.rules ?? []).map((r, idx) => (
                <div key={r.id} className="flex items-center gap-4 py-4 border-b border-[var(--border)] last:border-b-0">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2 flex-wrap">
                      <span className="text-sm font-medium text-[var(--text)]">{r.name}</span>
                      <span className="px-2 py-0.5 rounded text-[10px] uppercase tracking-wide bg-[var(--surface-hover)] text-[var(--text-muted)]">{r.scope}</span>
                      {r.id.startsWith("core-") && <span className="text-[10px] text-[var(--text-muted)]">(builtin)</span>}
                    </div>
                    {r.content && (
                      <p
                        role="button"
                        tabIndex={0}
                        className="text-xs text-[var(--text-muted)] mt-1.5 line-clamp-2 max-w-[420px] cursor-pointer hover:text-[var(--text)] hover:underline"
                        title={t("settingsAiClickToView")}
                        onClick={() => setViewingAiDetail({ kind: "rule", item: r })}
                        onKeyDown={(e) => e.key === "Enter" && setViewingAiDetail({ kind: "rule", item: r })}
                      >
                        {r.content.replace(/\s+/g, " ").trim()}
                      </p>
                    )}
                  </div>
                  <div className="flex items-center gap-3 flex-shrink-0">
                    <label className="flex items-center cursor-pointer select-none">
                      <span className={`relative inline-flex h-5 w-9 shrink-0 rounded-full transition-colors ${r.enabled ? "bg-primary" : "bg-[var(--surface-hover)]"}`}>
                        <input
                          type="checkbox"
                          checked={r.enabled}
                          onChange={(e) => {
                            const nextRules = [...(appSettings.ai?.rules ?? [])];
                            nextRules[idx] = { ...nextRules[idx], enabled: e.target.checked };
                            onAppSettingsChange({ ...appSettings, ai: { ...(appSettings.ai ?? { rules: [], skills: [], subagents: [], commands: [] }), rules: nextRules } });
                          }}
                          className="sr-only"
                          aria-label={t("settingsAiEnabled")}
                        />
                        <span className={`pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow-sm transition-transform mt-0.5 ml-0.5 ${r.enabled ? "translate-x-4" : "translate-x-0"}`} />
                      </span>
                    </label>
                    <button
                      type="button"
                      className="p-2 rounded-md text-[var(--text-muted)] hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
                      title={t("settingsAiEdit")}
                      aria-label={t("settingsAiEdit")}
                      onClick={() => setEditingAiItem({ kind: "rule", item: r })}
                    >
                      <Pencil size={14} />
                    </button>
                  </div>
                </div>
              ))}
              {(appSettings.ai?.rules ?? []).length === 0 && (
                <div className="py-6 text-center text-sm text-[var(--text-muted)]">{t("settingsAiNoRules")}</div>
              )}
            </div>
          </div>

          <div className="rounded-lg bg-[var(--surface-muted)]/20 p-4">
            <h4 className="text-sm font-semibold text-[var(--text)] mb-1">{t("settingsSubsectionAiSkills")}</h4>
            <p className="text-xs text-[var(--text-muted)] mb-4">{t("settingsAiSkillsDesc")}</p>
            <div className="space-y-0">
              {(appSettings.ai?.skills ?? []).map((s) => (
                <div key={s.id} className="flex items-center gap-4 py-4 border-b border-[var(--border)] last:border-b-0">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2 flex-wrap">
                      <span className="text-sm font-medium text-[var(--text)]">{s.name}</span>
                      <span className="px-2 py-0.5 rounded text-[10px] uppercase tracking-wide bg-[var(--surface-hover)] text-[var(--text-muted)]">{s.scope}</span>
                      {s.id.startsWith("core-") && <span className="text-[10px] text-[var(--text-muted)]">(builtin)</span>}
                    </div>
                    {(s.description ?? s.content) && (
                      <p
                        role="button"
                        tabIndex={0}
                        className="text-xs text-[var(--text-muted)] mt-1.5 line-clamp-2 max-w-[420px] cursor-pointer hover:text-[var(--text)] hover:underline"
                        title={t("settingsAiClickToView")}
                        onClick={() => setViewingAiDetail({ kind: "skill", item: s })}
                        onKeyDown={(e) => e.key === "Enter" && setViewingAiDetail({ kind: "skill", item: s })}
                      >
                        {(s.description ?? s.content ?? "").replace(/\s+/g, " ").trim()}
                      </p>
                    )}
                  </div>
                  <div className="flex items-center gap-3 flex-shrink-0">
                    <label className="flex items-center cursor-pointer select-none">
                      <span className={`relative inline-flex h-5 w-9 shrink-0 rounded-full transition-colors ${s.enabled ? "bg-primary" : "bg-[var(--surface-hover)]"}`}>
                        <input
                          type="checkbox"
                          checked={s.enabled}
                          onChange={(e) => {
                            const nextSkills = [...(appSettings.ai?.skills ?? [])];
                            const idx = nextSkills.findIndex((x) => x.id === s.id);
                            if (idx >= 0) {
                              nextSkills[idx] = { ...nextSkills[idx], enabled: e.target.checked };
                              onAppSettingsChange({ ...appSettings, ai: { ...(appSettings.ai ?? { rules: [], skills: [], subagents: [], commands: [] }), skills: nextSkills } });
                            }
                          }}
                          className="sr-only"
                          aria-label={t("settingsAiEnabled")}
                        />
                        <span className={`pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow-sm transition-transform mt-0.5 ml-0.5 ${s.enabled ? "translate-x-4" : "translate-x-0"}`} />
                      </span>
                    </label>
                    <button
                      type="button"
                      className="p-2 rounded-md text-[var(--text-muted)] hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
                      title={t("settingsAiEdit")}
                      aria-label={t("settingsAiEdit")}
                      onClick={() => setEditingAiItem({ kind: "skill", item: s })}
                    >
                      <Pencil size={14} />
                    </button>
                  </div>
                </div>
              ))}
              {(appSettings.ai?.skills ?? []).length === 0 && (
                <div className="py-6 text-center text-sm text-[var(--text-muted)]">{t("settingsAiNoSkills")}</div>
              )}
            </div>
          </div>

          <div className="rounded-lg bg-[var(--surface-muted)]/20 p-4">
            <h4 className="text-sm font-semibold text-[var(--text)] mb-1">{t("settingsSubsectionAiSubagents")}</h4>
            <p className="text-xs text-[var(--text-muted)] mb-4">{t("settingsAiSubagentsDesc")}</p>
            <div className="space-y-0">
              {(appSettings.ai?.subagents ?? []).map((a) => (
                <div key={a.id} className="flex items-center gap-4 py-4 border-b border-[var(--border)] last:border-b-0">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2 flex-wrap">
                      <span className="text-sm font-medium text-[var(--text)]">{a.name}</span>
                      <span className="px-2 py-0.5 rounded text-[10px] uppercase tracking-wide bg-[var(--surface-hover)] text-[var(--text-muted)]">{a.scope}</span>
                      {a.id.startsWith("core-") && <span className="text-[10px] text-[var(--text-muted)]">(builtin)</span>}
                    </div>
                    {(a.description ?? a.content) && (
                      <p
                        role="button"
                        tabIndex={0}
                        className="text-xs text-[var(--text-muted)] mt-1.5 line-clamp-2 max-w-[420px] cursor-pointer hover:text-[var(--text)] hover:underline"
                        title={t("settingsAiClickToView")}
                        onClick={() => setViewingAiDetail({ kind: "subagent", item: a })}
                        onKeyDown={(e) => e.key === "Enter" && setViewingAiDetail({ kind: "subagent", item: a })}
                      >
                        {(a.description ?? a.content ?? "").replace(/\s+/g, " ").trim()}
                      </p>
                    )}
                  </div>
                  <div className="flex items-center gap-3 flex-shrink-0">
                    <label className="flex items-center cursor-pointer select-none">
                      <span className={`relative inline-flex h-5 w-9 shrink-0 rounded-full transition-colors ${a.enabled ? "bg-primary" : "bg-[var(--surface-hover)]"}`}>
                        <input
                          type="checkbox"
                          checked={a.enabled}
                          onChange={(e) => {
                            const next = [...(appSettings.ai?.subagents ?? [])];
                            const idx = next.findIndex((x) => x.id === a.id);
                            if (idx >= 0) {
                              next[idx] = { ...next[idx], enabled: e.target.checked };
                              onAppSettingsChange({ ...appSettings, ai: { ...(appSettings.ai ?? { rules: [], skills: [], subagents: [], commands: [] }), subagents: next } });
                            }
                          }}
                          className="sr-only"
                          aria-label={t("settingsAiEnabled")}
                        />
                        <span className={`pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow-sm transition-transform mt-0.5 ml-0.5 ${a.enabled ? "translate-x-4" : "translate-x-0"}`} />
                      </span>
                    </label>
                    <button
                      type="button"
                      className="p-2 rounded-md text-[var(--text-muted)] hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
                      title={t("settingsAiEdit")}
                      aria-label={t("settingsAiEdit")}
                      onClick={() => setEditingAiItem({ kind: "subagent", item: a })}
                    >
                      <Pencil size={14} />
                    </button>
                  </div>
                </div>
              ))}
              {(appSettings.ai?.subagents ?? []).length === 0 && (
                <div className="py-6 text-center text-sm text-[var(--text-muted)]">{t("settingsAiNoSubagents")}</div>
              )}
            </div>
          </div>

          <div className="rounded-lg bg-[var(--surface-muted)]/20 p-4">
            <h4 className="text-sm font-semibold text-[var(--text)] mb-1">{t("settingsSubsectionAiCommands")}</h4>
            <p className="text-xs text-[var(--text-muted)] mb-4">{t("settingsAiCommandsDesc")}</p>
            <div className="space-y-0">
              {(appSettings.ai?.commands ?? []).map((c) => (
                <div key={c.id} className="flex items-center gap-4 py-4 border-b border-[var(--border)] last:border-b-0">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2 flex-wrap">
                      <span className="text-sm font-medium text-[var(--text)]">{c.name}</span>
                      <span className="px-2 py-0.5 rounded text-[10px] uppercase tracking-wide bg-[var(--surface-hover)] text-[var(--text-muted)]">{c.scope}</span>
                      {c.id.startsWith("core-") && <span className="text-[10px] text-[var(--text-muted)]">(builtin)</span>}
                    </div>
                    {(c.description ?? c.content) && (
                      <p
                        role="button"
                        tabIndex={0}
                        className="text-xs text-[var(--text-muted)] mt-1.5 line-clamp-2 max-w-[420px] cursor-pointer hover:text-[var(--text)] hover:underline"
                        title={t("settingsAiClickToView")}
                        onClick={() => setViewingAiDetail({ kind: "command", item: c })}
                        onKeyDown={(e) => e.key === "Enter" && setViewingAiDetail({ kind: "command", item: c })}
                      >
                        {(c.description ?? c.content ?? "").replace(/\s+/g, " ").trim()}
                      </p>
                    )}
                  </div>
                  <div className="flex items-center gap-3 flex-shrink-0">
                    <label className="flex items-center cursor-pointer select-none">
                      <span className={`relative inline-flex h-5 w-9 shrink-0 rounded-full transition-colors ${c.enabled ? "bg-primary" : "bg-[var(--surface-hover)]"}`}>
                        <input
                          type="checkbox"
                          checked={c.enabled}
                          onChange={(e) => {
                            const next = [...(appSettings.ai?.commands ?? [])];
                            const idx = next.findIndex((x) => x.id === c.id);
                            if (idx >= 0) {
                              next[idx] = { ...next[idx], enabled: e.target.checked };
                              onAppSettingsChange({ ...appSettings, ai: { ...(appSettings.ai ?? { rules: [], skills: [], subagents: [], commands: [] }), commands: next } });
                            }
                          }}
                          className="sr-only"
                          aria-label={t("settingsAiEnabled")}
                        />
                        <span className={`pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow-sm transition-transform mt-0.5 ml-0.5 ${c.enabled ? "translate-x-4" : "translate-x-0"}`} />
                      </span>
                    </label>
                    <button
                      type="button"
                      className="p-2 rounded-md text-[var(--text-muted)] hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
                      title={t("settingsAiEdit")}
                      aria-label={t("settingsAiEdit")}
                      onClick={() => setEditingAiItem({ kind: "command", item: c })}
                    >
                      <Pencil size={14} />
                    </button>
                  </div>
                </div>
              ))}
              {(appSettings.ai?.commands ?? []).length === 0 && (
                <div className="py-6 text-center text-sm text-[var(--text-muted)]">{t("settingsAiNoCommands")}</div>
              )}
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
          <SettingsSwitch checked={appSettings.documentation?.showWelcomeOnFirstLaunch !== false} onChange={(v) => onAppSettingsChange({ ...appSettings, documentation: { ...appSettings.documentation, showWelcomeOnFirstLaunch: v } })} ariaLabel={t("settingsShowWelcome")} />
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
