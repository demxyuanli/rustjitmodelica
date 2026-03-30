import { Check, Trash2 } from "lucide-react";
import type { AiConfig } from "../../api/tauri";
import { t } from "../../i18n";
import { BUILTIN_AI_MODELS, filterEnabledModels } from "../../constants/aiModels";
import type { AppSettingsForm } from "./settingsTypes";
import { SettingsSwitch } from "./settingsPrimitives";

export interface SettingsAiModelsSectionProps {
  apiKeyInput: string;
  setApiKeyInput: (v: string) => void;
  apiKeySaved: boolean;
  apiKeyBanner: string | null;
  grokApiKeyInput: string;
  setGrokApiKeyInput: (v: string) => void;
  grokApiKeySaved: boolean;
  grokApiKeyBanner: string | null;
  onSaveApiKey: () => void;
  onClearApiKey: () => void;
  onSaveGrokApiKey: () => void;
  onClearGrokApiKey: () => void;
  appSettings?: AppSettingsForm;
  onAppSettingsChange?: (s: AppSettingsForm) => void;
  aiModel?: string;
  onAiModelChange?: (model: string) => void;
}

export function SettingsAiModelsSection({
  apiKeyInput,
  setApiKeyInput,
  apiKeySaved,
  apiKeyBanner,
  grokApiKeyInput,
  setGrokApiKeyInput,
  grokApiKeySaved,
  grokApiKeyBanner,
  onSaveApiKey,
  onClearApiKey,
  onSaveGrokApiKey,
  onClearGrokApiKey,
  appSettings,
  onAppSettingsChange,
  aiModel,
  onAiModelChange,
}: SettingsAiModelsSectionProps) {
  const emptyAi = (): AiConfig => ({ rules: [], skills: [], subagents: [], commands: [] });

  return (
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
              <button type="button" onClick={onClearApiKey}
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
                onKeyDown={(e) => { if (e.key === "Enter") onSaveApiKey(); }}
                className="w-48 bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded" />
              <button type="button" onClick={onSaveApiKey} disabled={!apiKeyInput.trim()}
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
              <button type="button" onClick={onClearGrokApiKey}
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
                onKeyDown={(e) => { if (e.key === "Enter") onSaveGrokApiKey(); }}
                className="w-48 bg-[var(--surface)] border border-border px-2.5 py-1.5 text-sm rounded" />
              <button type="button" onClick={onSaveGrokApiKey} disabled={!grokApiKeyInput.trim()}
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
                            ...(appSettings.ai ?? emptyAi()),
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
  );
}
