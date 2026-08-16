import { useEffect, useRef, useState, type ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import type { MessageKey } from "../i18n";
import { t } from "../i18n";

export function MenuDrop({
  labelKey,
  children,
}: {
  labelKey: MessageKey;
  children: (close: () => void) => ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const close = () => setOpen(false);

  useEffect(() => {
    if (!open) return;
    const onDoc = (ev: MouseEvent) => {
      if (!rootRef.current?.contains(ev.target as Node)) setOpen(false);
    };
    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    window.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div className="menu-drop" ref={rootRef}>
      <button
        type="button"
        className={`menu-drop-label${open ? " open" : ""}`}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
      >
        {t(labelKey)}
      </button>
      {open ? (
        <div className="menu-drop-panel" role="menu">
          {children(close)}
        </div>
      ) : null}
    </div>
  );
}

export function MenuDropItem({
  icon: Icon,
  labelKey,
  accel,
  disabled,
  onClick,
}: {
  icon: LucideIcon;
  labelKey: MessageKey;
  accel?: string;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className="menu-drop-item"
      role="menuitem"
      disabled={disabled}
      onClick={onClick}
    >
      <Icon size={14} strokeWidth={1.75} />
      <span className="menu-drop-item-label">{t(labelKey)}</span>
      {accel ? <span className="menu-drop-accel">{accel}</span> : null}
    </button>
  );
}

export function MenuDropSep() {
  return <div className="menu-drop-sep" role="separator" />;
}
