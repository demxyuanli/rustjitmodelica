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
  Move,
  RotateCw,
  FlipHorizontal,
  FlipVertical,
  Focus,
} from "lucide-react";
import { t } from "../../i18n";
import type { GraphicItem } from "../diagramGraphicTypes";
import type { DiagramLayoutKind } from "../../utils/diagramLayout";
import { createDefaultGraphic } from "../ModelicaPropertyPanel";
import type { JointPaperHandle } from "../../utils/jointUtils";
import type { StructureSnapMode } from "../../structureEditor/session";

const SHAPE_ITEMS: Array<{ kind: GraphicItem["type"]; Icon: typeof Square; titleKey: string }> = [
  { kind: "Rectangle", Icon: Square, titleKey: "shapeRect" },
  { kind: "Ellipse", Icon: Circle, titleKey: "shapeEllipse" },
  { kind: "Line", Icon: Minus, titleKey: "shapeLine" },
  { kind: "Polygon", Icon: Hexagon, titleKey: "shapePolygon" },
  { kind: "Text", Icon: Type, titleKey: "shapeText" },
  { kind: "BSpline", Icon: Move, titleKey: "shapeBSpline" },
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
  structureToolbar?: {
    snapMode: StructureSnapMode;
    onSnapMode: (m: StructureSnapMode) => void;
    gridSize: number;
    onGridSize: (n: number) => void;
    onRotate90: () => void;
    onFlipH: () => void;
    onFlipV: () => void;
    pointerText: string | null;
    /** Element ids only (excludes connection cell ids). */
    selectedElementIds: string[];
  };
}

export function DiagramToolbar({
  readOnly,
  hasNodes,
  onAddGraphic,
  onApplyLayout,
  paperHandle,
  showMiniMap = true,
  onToggleMiniMap,
  structureToolbar,
}: DiagramToolbarProps) {
  const showShapes = !readOnly && onAddGraphic;
  const showLayout = !readOnly && hasNodes && onApplyLayout;
  const showZoom = Boolean(paperHandle);
  const showMinimapToggle = onToggleMiniMap !== undefined;
  const showStructure = Boolean(structureToolbar && !readOnly);

  if (!showShapes && !showLayout && !showZoom && !showMinimapToggle && !showStructure) return null;

  return (
    <div className="panel-header-bar shrink-0 flex items-center border-b border-[var(--border)] bg-[var(--bg-elevated)]">
      {showShapes && (
        <>
          <div className="flex items-center gap-0.5 rounded border border-[var(--border)] bg-[var(--surface)] p-0.5">
            {SHAPE_ITEMS.map(({ kind, Icon, titleKey }) => (
              <button
                key={kind}
                type="button"
                className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
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
          <div className="flex items-center gap-[var(--toolbar-gap)]">
            <span className="text-[10px] text-[var(--text-muted)] uppercase tracking-wide">
              {t("diagramAutoLayout")}
            </span>
            <div className="flex items-center gap-0.5 rounded border border-[var(--border)] bg-[var(--surface)] p-0.5">
              {LAYOUT_OPTIONS.map(({ kind, labelKey, Icon }) => (
                <button
                  key={kind}
                  type="button"
                  className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
                  onClick={() => onApplyLayout?.(kind)}
                  title={t(labelKey)}
                >
                  <Icon className="h-4 w-4" />
                </button>
              ))}
            </div>
          </div>
          {(showZoom || showMinimapToggle || showStructure) && <ToolbarSeparator />}
        </>
      )}
      {showStructure && structureToolbar && (
        <>
          <div className="flex items-center gap-1 text-[10px] text-[var(--text-muted)] uppercase tracking-wide">
            <span>{t("diagramSnapMode")}</span>
            <select
              value={structureToolbar.snapMode}
              onChange={(e) => structureToolbar.onSnapMode(e.target.value as StructureSnapMode)}
              className="rounded bg-[var(--surface)] border border-[var(--border)] px-1 py-0.5 text-[10px] normal-case"
            >
              <option value="none">{t("diagramSnapNone")}</option>
              <option value="grid">{t("diagramSnapGrid")}</option>
              <option value="gridAndGuide">{t("diagramSnapGridGuides")}</option>
            </select>
            <span className="ml-1">{t("diagramGridSize")}</span>
            <select
              value={structureToolbar.gridSize}
              onChange={(e) => structureToolbar.onGridSize(Number(e.target.value))}
              className="rounded bg-[var(--surface)] border border-[var(--border)] px-1 py-0.5 text-[10px] normal-case"
            >
              <option value={5}>5</option>
              <option value={10}>10</option>
              <option value={20}>20</option>
            </select>
          </div>
          <div className="flex items-center gap-0.5 rounded border border-[var(--border)] bg-[var(--surface)] p-0.5">
            <button
              type="button"
              className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
              onClick={() => structureToolbar.onRotate90()}
              title={t("diagramRotate90")}
            >
              <RotateCw className="h-4 w-4" />
            </button>
            <button
              type="button"
              className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
              onClick={() => structureToolbar.onFlipH()}
              title={t("diagramFlipH")}
            >
              <FlipHorizontal className="h-4 w-4" />
            </button>
            <button
              type="button"
              className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
              onClick={() => structureToolbar.onFlipV()}
              title={t("diagramFlipV")}
            >
              <FlipVertical className="h-4 w-4" />
            </button>
            <button
              type="button"
              className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
              onClick={() => paperHandle?.zoomToElementIds(structureToolbar.selectedElementIds)}
              title={t("diagramZoomSelection")}
              disabled={!structureToolbar.selectedElementIds.length}
            >
              <Focus className="h-4 w-4" />
            </button>
            <button
              type="button"
              className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
              onClick={() => paperHandle?.resetZoom100()}
              title={t("diagramZoom100")}
            >
              <span className="text-[10px] font-mono px-0.5">1:1</span>
            </button>
          </div>
          {structureToolbar.pointerText && (
            <span className="text-[10px] font-mono text-[var(--text-muted)] ml-1">
              {t("diagramPointerXY")}: {structureToolbar.pointerText}
            </span>
          )}
          {(showZoom || showMinimapToggle) && <ToolbarSeparator />}
        </>
      )}
      {showZoom && (
        <>
          <div className="flex items-center gap-0.5 rounded border border-[var(--border)] bg-[var(--surface)] p-0.5">
            <button
              type="button"
              className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
              onClick={() => paperHandle?.zoomIn()}
              title={t("zoomIn")}
            >
              <ZoomIn className="h-4 w-4" />
            </button>
            <button
              type="button"
              className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
              onClick={() => paperHandle?.zoomOut()}
              title={t("zoomOut")}
            >
              <ZoomOut className="h-4 w-4" />
            </button>
            <button
              type="button"
              className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
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
          className={`toolbar-icon-btn flex rounded items-center justify-center ${showMiniMap ? "bg-primary/20 text-primary" : "text-[var(--text-muted)] hover:text-[var(--text)]"} hover:bg-white/10`}
          onClick={onToggleMiniMap}
          title={showMiniMap ? t("hideMinimap") : t("showMinimap")}
        >
          <Map className="h-4 w-4" />
        </button>
      )}
    </div>
  );
}
