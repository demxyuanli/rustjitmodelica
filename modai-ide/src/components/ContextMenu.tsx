import { useEffect } from "react";

export interface ContextMenuItem {
  id: string;
  label: string;
  disabled?: boolean;
  onClick: () => void;
}

export interface ContextMenuProps {
  visible: boolean;
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}

export function ContextMenu({ visible, x, y, items, onClose }: ContextMenuProps) {
  useEffect(() => {
    if (!visible) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    const onClick = () => {
      onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("click", onClick);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("click", onClick);
    };
  }, [visible, onClose]);

  if (!visible) return null;

  return (
    <div
      className="fixed z-50 min-w-[200px] max-w-[320px] rounded border border-border bg-[var(--menu-bg)] text-[var(--text)] shadow-lg py-1 text-sm"
      style={{ top: y, left: x }}
      onContextMenu={(event) => {
        event.preventDefault();
      }}
    >
      {items.map((item) => (
        <button
          key={item.id}
          type="button"
          className="w-full text-left px-3 py-1.5 text-sm text-[var(--text)] hover:bg-[var(--menu-hover)] disabled:opacity-50 disabled:hover:bg-transparent"
          onClick={() => {
            if (item.disabled) return;
            item.onClick();
            onClose();
          }}
          disabled={item.disabled}
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}

