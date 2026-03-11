import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";

export interface IndexActionState {
  running: boolean;
  action: "refresh" | "rebuild" | null;
  done: number;
  total: number;
}

export interface SettingsContentProps {
  theme: "dark" | "light";
  onThemeChange: (theme: "dark" | "light") => void;
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
}: SettingsContentProps) {
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [apiKeySaved, setApiKeySaved] = useState(false);
  const [apiKeyBanner, setApiKeyBanner] = useState<string | null>(null);

  useEffect(() => {
    invoke<string>("get_api_key")
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
      await invoke("set_api_key", { apiKey: apiKeyInput.trim() });
      setApiKeySaved(true);
      setApiKeyInput("");
      setApiKeyBanner(t("apiKeySaveSuccess"));
    } catch (e) {
      setApiKeyBanner(String(e));
    }
  }, [apiKeyInput]);

  const handleClearApiKey = useCallback(async () => {
    try {
      await invoke("set_api_key", { apiKey: "" });
      setApiKeySaved(false);
      setApiKeyBanner(t("apiKeyClearSuccess"));
    } catch (e) {
      setApiKeyBanner(String(e));
    }
  }, []);

  const indexPct = indexState === "ready" && indexFileCount > 0 ? 100 : (indexAction?.total ? Math.round((indexAction.done / indexAction.total) * 100) : 0);

  return (
    <div className="space-y-6 text-[var(--text)]">
      <section>
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
                <input type="password" placeholder="API key" value={apiKeyInput}
                  onChange={(e) => setApiKeyInput(e.target.value)}
                  onKeyDown={(e) => { if (e.key === "Enter") handleSaveApiKey(); }}
                  className="w-40 bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded" />
                <button type="button" onClick={handleSaveApiKey} disabled={!apiKeyInput.trim()}
                  className="px-3 py-1.5 bg-primary hover:bg-blue-600 text-sm rounded text-white disabled:opacity-40">
                  Save
                </button>
              </div>
            )}
          </SettingsRow>
        </div>
      </section>

      {onAiModelChange && aiDailyLimit != null && (
        <section>
          <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionDeveloper")}</h3>
          <SettingsRow title="AI model" description="Model and usage for AI coding assistant.">
            <select
              value={aiModel || "deepseek-coder-v2"}
              onChange={(e) => onAiModelChange(e.target.value)}
              className="bg-[var(--surface)] border border-border px-2.5 py-1.5 text-xs rounded text-[var(--text)]"
            >
              <option value="deepseek-coder-v2">deepseek-coder-v2</option>
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
                Reset
              </button>
            )}
          </SettingsRow>
        </section>
      )}

      <section>
        <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionAppearance")}</h3>
        <SettingsRow title={t("settingsAppearance")} description={t("settingsAppearanceDesc")}>
          <div className="flex rounded-md overflow-hidden border border-border">
            <button type="button" className={`px-3 py-1.5 text-xs ${theme === "dark" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onThemeChange("dark")}>{t("themeDark")}</button>
            <button type="button" className={`px-3 py-1.5 text-xs ${theme === "light" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onThemeChange("light")}>{t("themeLight")}</button>
          </div>
        </SettingsRow>
      </section>

      <section>
        <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionShortcuts")}</h3>
        <SettingsRow title={t("settingsKeyboard")} description={t("settingsKeyboardDesc")}>
          <div className="text-right text-xs space-y-1">
            <div className="flex justify-end gap-2"><span className="font-mono bg-[var(--surface)] px-2 py-0.5 rounded">{t("save")}</span><span className="text-[var(--text-muted)]">Ctrl+S</span></div>
            <div className="flex justify-end gap-2"><span className="font-mono bg-[var(--surface)] px-2 py-0.5 rounded">{t("toggleSidebar")}</span><span className="text-[var(--text-muted)]">Ctrl+B</span></div>
            <div className="flex justify-end gap-2"><span className="font-mono bg-[var(--surface)] px-2 py-0.5 rounded">{t("toggleBottomPanel")}</span><span className="text-[var(--text-muted)]">Ctrl+J</span></div>
          </div>
        </SettingsRow>
      </section>

      <section>
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

      {onEnterDevMode && (
        <section>
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
  );
}
