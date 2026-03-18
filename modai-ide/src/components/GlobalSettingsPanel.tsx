import { t } from "../i18n";
import { SettingsContent, type SettingsContentProps } from "./SettingsContent";

export interface GlobalSettingsPanelProps extends SettingsContentProps {
  open: boolean;
  onClose: () => void;
  initialGroupId?: string | null;
}

/**
 * Single global settings panel: right-side floating, used once in App.
 * Not implemented per-workspace.
 */
export function GlobalSettingsPanel({ open, onClose, initialGroupId, ...settingsProps }: GlobalSettingsPanelProps) {
  if (!open) return null;
  return (
    <div
      className="absolute right-0 top-0 bottom-0 z-50 flex w-1/2 flex-col border-l border-border bg-[var(--surface)] overflow-hidden shadow-lg"
      aria-label={t("settings")}
      role="dialog"
    >
      <div className="panel-header-bar shrink-0 flex items-center justify-between border-b border-border">
        <h2 className="text-lg font-semibold text-[var(--text)]">{t("settings")}</h2>
        <button
          type="button"
          onClick={onClose}
          className="p-2 rounded text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-[var(--surface-hover)]"
          title={t("closeSettings")}
          aria-label={t("closeSettings")}
        >
          <span aria-hidden="true">&#215;</span>
        </button>
      </div>
      <div className="flex-1 min-h-0 overflow-auto">
        <div className="p-4">
          <SettingsContent {...settingsProps} initialGroupId={initialGroupId} onClose={onClose} />
        </div>
      </div>
    </div>
  );
}
