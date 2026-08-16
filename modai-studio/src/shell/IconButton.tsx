import type { LucideIcon } from "lucide-react";
import type { MessageKey } from "../i18n";
import { t } from "../i18n";

export function IconButton({
  icon: Icon,
  labelKey,
  tipKey,
  onClick,
  disabled,
  primary,
  showLabel = true,
  showTip = true,
  active,
  className,
  title: titleOverride,
}: {
  icon: LucideIcon;
  labelKey: MessageKey;
  tipKey?: MessageKey;
  onClick?: () => void;
  disabled?: boolean;
  primary?: boolean;
  showLabel?: boolean;
  showTip?: boolean;
  active?: boolean;
  className?: string;
  title?: string;
}) {
  const label = t(labelKey);
  const tip = titleOverride ?? (tipKey ? t(tipKey) : label);
  return (
    <button
      type="button"
      className={`icon-btn${primary ? " primary" : ""}${active ? " active" : ""}${className ? ` ${className}` : ""}`}
      onClick={onClick}
      disabled={disabled}
      title={showTip ? tip : undefined}
      aria-label={label}
    >
      <Icon size={16} strokeWidth={1.75} />
      {showLabel ? <span className="icon-btn-label">{label}</span> : null}
    </button>
  );
}
