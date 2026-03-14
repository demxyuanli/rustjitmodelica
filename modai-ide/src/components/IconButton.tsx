import type { ButtonHTMLAttributes, ReactNode } from "react";

type IconButtonVariant = "ghost" | "primary" | "tab";
type IconButtonSize = "xs" | "sm";

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  icon: ReactNode;
  variant?: IconButtonVariant;
  size?: IconButtonSize;
  active?: boolean;
}

export function IconButton({
  icon,
  variant = "ghost",
  size = "sm",
  active = false,
  className,
  ...rest
}: IconButtonProps) {
  const base =
    "inline-flex items-center justify-center rounded focus:outline-none focus-visible:ring-1 focus-visible:ring-[var(--accent)] disabled:opacity-50 disabled:cursor-default";
  const sizeClass = size === "xs" ? "h-6 w-6 text-xs" : "h-7 w-7 text-sm";

  let variantClass = "";
  if (variant === "primary") {
    variantClass = active
      ? "bg-primary text-white"
      : "bg-primary text-white hover:bg-blue-600";
  } else if (variant === "tab") {
    variantClass = active
      ? "bg-[var(--surface-active)] text-[var(--text)]"
      : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)]";
  } else {
    variantClass = active
      ? "bg-[var(--surface-active)] text-[var(--text)]"
      : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)] hover:text-[var(--text)]";
  }

  const merged = className ? `${base} ${sizeClass} ${variantClass} ${className}` : `${base} ${sizeClass} ${variantClass}`;

  return (
    <button type="button" className={merged} {...rest}>
      {icon}
    </button>
  );
}

