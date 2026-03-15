import {
  Square,
  Circle,
  Minus,
  Hexagon,
  Type,
  LayoutGrid,
  Layers,
  Circle as CircleIcon,
  Network,
  ArrowRightLeft,
  ArrowUpDown,
  ZoomIn,
  ZoomOut,
  Maximize2,
  Map,
} from "lucide-react";
import { t } from "../../i18n";
import type { GraphicItem } from "../DiagramSvgRenderer";
import type { DiagramLayoutKind } from "../../utils/diagramLayout";
import { createDefaultGraphic } from "../ModelicaPropertyPanel";
import type { JointPaperHandle } from "../../utils/jointUtils";

const SHAPE_ITEMS: Array<{ kind: GraphicItem["type"]; Icon: typeof Square; titleKey: string }> = [
  { kind: "Rectangle", Icon: Square, titleKey: "shapeRect" },
  { kind: "Ellipse", Icon: Circle, titleKey: "shapeEllipse" },
  { kind: "Line", Icon: Minus, titleKey: "shapeLine" },
  { kind: "Polygon", Icon: Hexagon, titleKey: "shapePolygon" },
  { kind: "Text", Icon: Type, titleKey: "shapeText" },
];

const LAYOUT_OPTIONS: Array<{ kind: DiagramLayoutKind; labelKey: string; Icon: typeof LayoutGrid }> = [
  { kind: "grid", labelKey: "diagramLayoutGrid", Icon: LayoutGrid },
  { kind: "hierarchical", labelKey: "diagramLayoutHierarchical", Icon: Layers },
  { kind: "circular", labelKey: "diagramLayoutCircular", Icon: CircleIcon },
  { kind: "force", labelKey: "diagramLayoutForce", Icon: Network },
  { kind: "horizontal", labelKey: "diagramLayoutHorizontal", Icon: ArrowRightLeft },
  { kind: "vertical", labelKey: "diagramLayoutVertical", Icon: ArrowUpDown },
];

function ToolbarSeparator() {
  return <div className="w-px h-5 bg-[var(--border)]" />;
}

export interface DiagramToolbarProps {
  readOnly?: boolean;
  hasNodes?: boolean;
  onAddGraphic?: (graphic: GraphicItem) => void;
  onApplyLayout?: (kind: DiagramLayoutKind) => void;
  paperHandle?: JointPaperHandle | null;
  showMiniMap?: boolean;
  onToggleMiniMap?: () => void;
}

export function DiagramToolbar({
  readOnly,
  hasNodes,
  onAddGraphic,
  onApplyLayout,
  paperHandle,
  showMiniMap = true,
  onToggleMiniMap,
}: DiagramToolbarProps) {
  const showShapes = !readOnly && onAddGraphic;
  const showLayout = !readOnly && hasNodes && onApplyLayout;
  const showZoom = Boolean(paperHandle);
  const showMinimapToggle = onToggleMiniMap !== undefined;

  if (!showShapes && !showLayout && !showZoom && !showMinimapToggle) return null;

  return (
    <div className="shrink-0 flex items-center gap-2 px-2 py-1 border-b border-[var(--border)] bg-[var(--bg-elevated)]">
      {showShapes && (
        <>
          <div className="flex items-center gap-0.5 rounded border border-[var(--border)] bg-[var(--surface)] p-0.5">
            {SHAPE_ITEMS.map(({ kind, Icon, titleKey }) => (
              <button
                key={kind}
                type="button"
                className="rounded p-1.5 text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
                onClick={() => onAddGraphic?.(createDefaultGraphic(kind))}
                title={t(titleKey)}
              >
                <Icon className="h-4 w-4" />
              </button>
            ))}
          </div>
          {(showLayout || showZoom || showMinimapToggle) && <ToolbarSeparator />}
        </>
      )}
      {showLayout && (
        <>
          <div className="flex items-center gap-1">
            <span className="text-[10px] text-[var(--text-muted)] uppercase tracking-wide">
              {t("diagramAutoLayout")}
            </span>
            <div className="flex items-center gap-0.5 rounded border border-[var(--border)] bg-[var(--surface)] p-0.5">
              {LAYOUT_OPTIONS.map(({ kind, labelKey, Icon }) => (
                <button
                  key={kind}
                  type="button"
                  className="rounded p-1.5 text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
                  onClick={() => onApplyLayout?.(kind)}
                  title={t(labelKey)}
                >
                  <Icon className="h-4 w-4" />
                </button>
              ))}
            </div>
          </div>
          {(showZoom || showMinimapToggle) && <ToolbarSeparator />}
        </>
      )}
      {showZoom && (
        <>
          <div className="flex items-center gap-0.5 rounded border border-[var(--border)] bg-[var(--surface)] p-0.5">
            <button
              type="button"
              className="rounded p-1.5 text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
              onClick={() => paperHandle?.zoomIn()}
              title={t("zoomIn")}
            >
              <ZoomIn className="h-4 w-4" />
            </button>
            <button
              type="button"
              className="rounded p-1.5 text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
              onClick={() => paperHandle?.zoomOut()}
              title={t("zoomOut")}
            >
              <ZoomOut className="h-4 w-4" />
            </button>
            <button
              type="button"
              className="rounded p-1.5 text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
              onClick={() => paperHandle?.fitView({ padding: 0.16 })}
              title={t("fitView")}
            >
              <Maximize2 className="h-4 w-4" />
            </button>
          </div>
          {showMinimapToggle && <ToolbarSeparator />}
        </>
      )}
      {showMinimapToggle && (
        <button
          type="button"
          className={`rounded p-1.5 ${showMiniMap ? "bg-primary/20 text-primary" : "text-[var(--text-muted)] hover:text-[var(--text)]"} hover:bg-white/10`}
          onClick={onToggleMiniMap}
          title={showMiniMap ? t("hideMinimap") : t("showMinimap")}
        >
          <Map className="h-4 w-4" />
        </button>
      )}
    </div>
  );
}
