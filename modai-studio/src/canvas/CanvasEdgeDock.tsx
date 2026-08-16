import { useState, type ReactNode } from "react";
import { ChevronLeft, ChevronRight, type LucideIcon } from "lucide-react";

export type CanvasEdgeDockId = "wires" | "sampleTime" | "minimap";

export function CanvasEdgeDock({
  id,
  side,
  label,
  icon: Icon,
  pinned,
  onPinnedChange,
  children,
}: {
  id: CanvasEdgeDockId;
  side: "left" | "right";
  label: string;
  icon: LucideIcon;
  pinned: boolean;
  onPinnedChange: (pinned: boolean) => void;
  children: ReactNode;
}) {
  const [hovered, setHovered] = useState(false);
  const open = pinned || hovered;
  const Chevron = side === "left" ? (open ? ChevronLeft : ChevronRight) : open ? ChevronRight : ChevronLeft;

  return (
    <div
      className={`canvas-edge-dock side-${side}${open ? " is-open" : ""}${pinned ? " is-pinned" : ""}`}
      data-dock={id}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onMouseDown={(ev) => ev.stopPropagation()}
      onClick={(ev) => ev.stopPropagation()}
    >
      <button
        type="button"
        className="canvas-edge-dock-tab"
        aria-expanded={open}
        aria-label={label}
        title={label}
        onClick={() => onPinnedChange(!pinned)}
      >
        <Icon size={14} strokeWidth={1.8} />
        <Chevron size={12} strokeWidth={2} />
      </button>
      <div className="canvas-edge-dock-body" aria-hidden={!open}>
        {children}
      </div>
    </div>
  );
}
