import type { ReactNode } from "react";
import { AppIcon } from "../Icon";

export interface SimulationTabButtonProps {
  active: boolean;
  label: string;
  icon: ReactNode;
  badge?: number;
  onClick: () => void;
}

export function SimulationTabButton({ active, label, icon, badge, onClick }: SimulationTabButtonProps) {
  return (
    <button
      type="button"
      className={`relative flex shrink-0 items-center gap-1.5 border-r border-border px-3 py-1.5 text-xs transition-colors ${
        active
          ? "border-b-2 border-b-primary -mb-px bg-surface text-[var(--text)] font-medium"
          : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)]"
      }`}
      onClick={onClick}
    >
      {icon}
      <span>{label}</span>
      {badge != null && badge > 0 && (
        <span className="ml-1 rounded bg-[var(--danger-text)]/20 px-1 text-[10px] font-medium tabular-nums text-[var(--danger-text)]">
          {badge}
        </span>
      )}
    </button>
  );
}

export interface SimulationSectionHeaderProps {
  title: string;
  expanded: boolean;
  onToggle: () => void;
  statusIcon?: ReactNode;
  badge?: ReactNode;
  toolbar?: ReactNode;
}

export function SimulationSectionHeader({
  title,
  expanded,
  onToggle,
  statusIcon,
  badge,
  toolbar,
}: SimulationSectionHeaderProps) {
  return (
    <div className="panel-header-bar flex shrink-0 items-center border-b border-border bg-surface-alt">
      <button
        type="button"
        className="flex flex-1 items-center gap-1.5 text-left text-[11px] font-semibold uppercase tracking-wide text-[var(--text-muted)]"
        onClick={onToggle}
      >
        <AppIcon
          name="next"
          className={`!h-3 !w-3 transition-transform ${expanded ? "rotate-90" : "rotate-0"}`}
        />
        {statusIcon}
        <span>{title}</span>
        {badge}
      </button>
      {toolbar && <div className="ml-auto flex items-center gap-[var(--toolbar-gap)]">{toolbar}</div>}
    </div>
  );
}
