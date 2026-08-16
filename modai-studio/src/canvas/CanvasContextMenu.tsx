import { BoxSelect, RotateCw, Spline, Trash2, Workflow } from "lucide-react";
import { t } from "../i18n";

export type CanvasMenuAction = "place" | "subsystem" | "plot" | "rotate" | "delete";

export function CanvasContextMenu({
  x,
  y,
  canPlace,
  canSubsystem,
  canPlot,
  canRotate,
  canDelete,
  onAction,
  onClose,
}: {
  x: number;
  y: number;
  canPlace: boolean;
  canSubsystem: boolean;
  canPlot: boolean;
  canRotate: boolean;
  canDelete: boolean;
  onAction: (a: CanvasMenuAction) => void;
  onClose: () => void;
}) {
  const items: { id: CanvasMenuAction; label: string; enabled: boolean; Icon: typeof RotateCw }[] = [
    { id: "place", label: t("ctxPlaceBlock"), enabled: canPlace, Icon: BoxSelect },
    { id: "subsystem", label: t("ctxCreateSubsystem"), enabled: canSubsystem, Icon: Workflow },
    { id: "plot", label: t("ctxPlotSignal"), enabled: canPlot, Icon: Spline },
    { id: "rotate", label: t("ctxRotate"), enabled: canRotate, Icon: RotateCw },
    { id: "delete", label: t("ctxDelete"), enabled: canDelete, Icon: Trash2 },
  ];

  return (
    <div
      className="canvas-ctx-backdrop"
      onClick={onClose}
      onContextMenu={(ev) => {
        ev.preventDefault();
        onClose();
      }}
    >
      <div
        className="canvas-ctx-menu"
        style={{ left: x, top: y }}
        onClick={(ev) => ev.stopPropagation()}
      >
        {items.map((it) => (
          <button
            key={it.id}
            type="button"
            disabled={!it.enabled}
            onClick={() => {
              if (!it.enabled) return;
              onAction(it.id);
              onClose();
            }}
          >
            <it.Icon size={14} strokeWidth={1.75} />
            {it.label}
          </button>
        ))}
      </div>
    </div>
  );
}
