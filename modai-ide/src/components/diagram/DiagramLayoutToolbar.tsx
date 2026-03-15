import { LayoutGrid, Layers, Circle, Network } from "lucide-react";
import { t } from "../../i18n";
import type { DiagramLayoutKind } from "../../utils/diagramLayout";

const LAYOUT_OPTIONS: { kind: DiagramLayoutKind; labelKey: string; Icon: typeof LayoutGrid }[] = [
  { kind: "grid", labelKey: "diagramLayoutGrid", Icon: LayoutGrid },
  { kind: "hierarchical", labelKey: "diagramLayoutHierarchical", Icon: Layers },
  { kind: "circular", labelKey: "diagramLayoutCircular", Icon: Circle },
  { kind: "force", labelKey: "diagramLayoutForce", Icon: Network },
];

export interface DiagramLayoutToolbarProps {
  onApplyLayout: (kind: DiagramLayoutKind) => void;
  disabled?: boolean;
}

export function DiagramLayoutToolbar({ onApplyLayout, disabled }: DiagramLayoutToolbarProps) {
  return (
    <div className="flex items-center gap-[var(--toolbar-gap)] shrink-0">
      <span className="text-[10px] text-[var(--text-muted)] uppercase tracking-wide mr-1">
        {t("diagramAutoLayout")}
      </span>
      <div className="flex items-center gap-0.5 rounded border border-[var(--border)] bg-[var(--bg-elevated)] p-0.5">
        {LAYOUT_OPTIONS.map(({ kind, labelKey, Icon }) => (
          <button
            key={kind}
            type="button"
            className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-[var(--surface)] disabled:opacity-50 disabled:pointer-events-none"
            onClick={() => onApplyLayout(kind)}
            disabled={disabled}
            title={t(labelKey)}
          >
            <Icon className="h-4 w-4" />
          </button>
        ))}
      </div>
    </div>
  );
}
