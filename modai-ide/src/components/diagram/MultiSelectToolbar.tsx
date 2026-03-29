/**
 * Multi-select toolbar for graphic editing
 * Provides selection and batch operations
 */

import { Copy, Trash2, Group, Ungroup } from "lucide-react";
import { t } from "../../i18n";

export interface MultiSelectToolbarProps {
  selectedIndices: number[];
  onGroup?: () => void;
  onUngroup?: () => void;
  onDuplicate: () => void;
  onDelete: () => void;
}

function ToolbarSeparator() {
  return <div className="w-px h-4 bg-[var(--border)]" />;
}

export function MultiSelectToolbar({
  selectedIndices,
  onGroup,
  onUngroup,
  onDuplicate,
  onDelete,
}: MultiSelectToolbarProps) {
  const hasSelection = selectedIndices.length >= 1;
  const canGroup = selectedIndices.length >= 2;
  const showGroupControls = canGroup && onGroup && onUngroup;

  if (!hasSelection) return null;

  return (
    <div className="flex items-center gap-0.5 rounded border border-[var(--border)] bg-[var(--surface)] p-0.5">
      <button
        type="button"
        className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
        onClick={onDuplicate}
        title={t("duplicate")}
        disabled={selectedIndices.length === 0}
      >
        <Copy className="h-4 w-4" />
      </button>
      {showGroupControls && (
        <>
          <button
            type="button"
            className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
            onClick={onGroup}
            title={t("group")}
          >
            <Group className="h-4 w-4" />
          </button>
          <button
            type="button"
            className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
            onClick={onUngroup}
            title={t("ungroup")}
          >
            <Ungroup className="h-4 w-4" />
          </button>
          <ToolbarSeparator />
        </>
      )}
      <button
        type="button"
        className="toolbar-icon-btn flex rounded items-center justify-center text-red-400 hover:text-red-300 hover:bg-white/10"
        onClick={onDelete}
        title={t("delete")}
      >
        <Trash2 className="h-4 w-4" />
      </button>
    </div>
  );
}
