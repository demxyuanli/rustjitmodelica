import type { Dispatch, SetStateAction } from "react";
import { Pencil } from "lucide-react";
import type { AiRule, AiSkill, AiSubagent, AiCommand } from "../../api/tauri";
import { t } from "../../i18n";
import type { AppSettingsForm } from "./settingsTypes";
import type { EditingAiItemState, ViewingAiDetailNonNull } from "./SettingsAiConfigModals";

export interface SettingsAiConfigSectionProps {
  appSettings: AppSettingsForm;
  onAppSettingsChange: (s: AppSettingsForm) => void;
  setEditingAiItem: Dispatch<SetStateAction<EditingAiItemState>>;
  setViewingAiDetail: Dispatch<SetStateAction<ViewingAiDetailNonNull | null>>;
}

export function SettingsAiConfigSection({
  appSettings,
  onAppSettingsChange,
  setEditingAiItem,
  setViewingAiDetail,
}: SettingsAiConfigSectionProps) {
  const emptyAi = () => ({ rules: [] as AiRule[], skills: [] as AiSkill[], subagents: [] as AiSubagent[], commands: [] as AiCommand[] });

  return (
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
                          onAppSettingsChange({ ...appSettings, ai: { ...(appSettings.ai ?? emptyAi()), rules: nextRules } });
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
                            onAppSettingsChange({ ...appSettings, ai: { ...(appSettings.ai ?? emptyAi()), skills: nextSkills } });
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
                            onAppSettingsChange({ ...appSettings, ai: { ...(appSettings.ai ?? emptyAi()), subagents: next } });
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
                            onAppSettingsChange({ ...appSettings, ai: { ...(appSettings.ai ?? emptyAi()), commands: next } });
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
  );
}
