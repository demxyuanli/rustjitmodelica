import type { Dispatch, SetStateAction } from "react";
import { Check, X } from "lucide-react";
import type { AiRule, AiSkill, AiSubagent, AiCommand } from "../../api/tauri";
import { t } from "../../i18n";
import type { AppSettingsForm } from "./settingsTypes";

export type EditingAiItemState =
  | { kind: "rule"; item: AiRule }
  | { kind: "skill"; item: AiSkill }
  | { kind: "subagent"; item: AiSubagent }
  | { kind: "command"; item: AiCommand }
  | null;

export type ViewingAiDetailNonNull =
  | { kind: "rule"; item: AiRule }
  | { kind: "skill"; item: AiSkill }
  | { kind: "subagent"; item: AiSubagent }
  | { kind: "command"; item: AiCommand };

export type ViewingAiDetailState = ViewingAiDetailNonNull | null;

export interface SettingsAiConfigModalsProps {
  editingAiItem: EditingAiItemState;
  setEditingAiItem: Dispatch<SetStateAction<EditingAiItemState>>;
  editingAiError: string | null;
  setEditingAiError: (v: string | null) => void;
  viewingAiDetail: ViewingAiDetailState;
  setViewingAiDetail: Dispatch<SetStateAction<ViewingAiDetailState>>;
  appSettings?: AppSettingsForm;
  onAppSettingsChange?: (s: AppSettingsForm) => void;
}

export function SettingsAiConfigModals({
  editingAiItem,
  setEditingAiItem,
  editingAiError,
  setEditingAiError,
  viewingAiDetail,
  setViewingAiDetail,
  appSettings,
  onAppSettingsChange,
}: SettingsAiConfigModalsProps) {
  return (
    <>
      {editingAiItem && appSettings && onAppSettingsChange && (
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
                    setEditingAiItem({ ...editingAiItem, item: { ...editingAiItem.item, name: e.target.value } as never });
                  }}
                />
              </div>
              {"description" in editingAiItem.item && (
                <div className="flex flex-col gap-1">
                  <label className="text-[var(--text-muted)]">{t("settingsAiEditDescription")}</label>
                  <input
                    className="bg-[var(--surface)] border border-border rounded px-2 py-1 text-xs text-[var(--text)]"
                    value={(editingAiItem.item as AiSkill).description ?? ""}
                    onChange={(e) => {
                      setEditingAiError(null);
                      setEditingAiItem({ ...editingAiItem, item: { ...(editingAiItem.item as unknown as Record<string, unknown>), description: e.target.value } as never });
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
                    setEditingAiItem({ ...editingAiItem, item: { ...editingAiItem.item, scope: e.target.value as never } as never });
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
                    setEditingAiItem({ ...editingAiItem, item: { ...editingAiItem.item, content: e.target.value } as never });
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
                  if (!editingAiItem) return;
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
              {"description" in viewingAiDetail.item && (viewingAiDetail.item as AiSkill).description && (
                <p className="text-xs text-[var(--text-muted)] mb-2">{(viewingAiDetail.item as AiSkill).description}</p>
              )}
              <pre className="text-xs text-[var(--text)] whitespace-pre-wrap break-words font-sans">{viewingAiDetail.item.content ?? ""}</pre>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
