import { useEffect, useId, useMemo, useRef, type PointerEvent as ReactPointerEvent } from "react";
import {
  AnnotationGraphicsSvg,
  findDeepestGraphicPath,
  getGraphicAtPath,
  svgToCoord,
  translateGraphicItem,
  type AnnotationPoint,
  type GraphicItem,
  type IconDiagramAnnotation,
} from "./DiagramSvgRenderer";
import { snapToGrid, type GridOptions } from "../utils/gridSnap";

function cloneGraphicItem(item: GraphicItem): GraphicItem {
  return structuredClone(item);
}

interface IconEditorViewProps {
  annotation: IconDiagramAnnotation;
  selectedGraphicPath: number[] | null;
  readOnly: boolean;
  gridEnabled?: boolean;
  gridSize?: number;
  showGrid?: boolean;
  onSelectGraphic: (path: number[] | null, additive?: boolean) => void;
  onUpdateGraphic: (path: number[], next: GraphicItem) => void;
}

function clientToModelPoint(
  event: ReactPointerEvent<SVGSVGElement>,
  svgElement: SVGSVGElement,
  annotation: IconDiagramAnnotation,
): AnnotationPoint {
  const rect = svgElement.getBoundingClientRect();
  return svgToCoord(
    {
      x: event.clientX - rect.left,
      y: event.clientY - rect.top,
    },
    annotation.coordinateSystem,
    rect.width,
    rect.height,
  );
}

export function IconEditorView({
  annotation,
  selectedGraphicPath,
  readOnly,
  gridEnabled = false,
  gridSize = 10,
  showGrid = false,
  onSelectGraphic,
  onUpdateGraphic,
}: IconEditorViewProps) {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const dragStateRef = useRef<{ path: number[]; lastPoint: AnnotationPoint } | null>(null);
  const dragWorkingRef = useRef<GraphicItem | null>(null);
  const pendingDeltaRef = useRef<AnnotationPoint>({ x: 0, y: 0 });
  const rafIdRef = useRef<number | null>(null);
  const onUpdateGraphicRef = useRef(onUpdateGraphic);
  onUpdateGraphicRef.current = onUpdateGraphic;
  const gridPatternId = useId().replace(/[^a-zA-Z0-9_-]/g, "_");

  const snapOptions = useMemo<GridOptions>(() => ({
    enabled: gridEnabled,
    gridSize,
    snapTolerance: gridSize / 2,
  }), [gridEnabled, gridSize]);

  const flushPendingDrag = () => {
    if (rafIdRef.current != null) {
      cancelAnimationFrame(rafIdRef.current);
      rafIdRef.current = null;
    }
    const path = dragStateRef.current?.path;
    const d = pendingDeltaRef.current;
    if (!path || path.length === 0 || (d.x === 0 && d.y === 0)) {
      pendingDeltaRef.current = { x: 0, y: 0 };
      return;
    }
    const working = dragWorkingRef.current;
    if (!working) return;
    pendingDeltaRef.current = { x: 0, y: 0 };
    dragWorkingRef.current = translateGraphicItem(working, d);
    onUpdateGraphicRef.current(path, dragWorkingRef.current);
  };

  const scheduleDragFlush = () => {
    if (rafIdRef.current != null) return;
    rafIdRef.current = requestAnimationFrame(() => {
      rafIdRef.current = null;
      const path = dragStateRef.current?.path;
      const d = pendingDeltaRef.current;
      if (!path || path.length === 0 || (d.x === 0 && d.y === 0)) {
        pendingDeltaRef.current = { x: 0, y: 0 };
        return;
      }
      const working = dragWorkingRef.current;
      if (!working) return;
      pendingDeltaRef.current = { x: 0, y: 0 };
      dragWorkingRef.current = translateGraphicItem(working, d);
      onUpdateGraphicRef.current(path, dragWorkingRef.current);
    });
  };

  useEffect(() => {
    const onWindowPointerUp = () => {
      flushPendingDrag();
      dragStateRef.current = null;
      dragWorkingRef.current = null;
    };
    window.addEventListener("pointerup", onWindowPointerUp);
    return () => {
      window.removeEventListener("pointerup", onWindowPointerUp);
      if (rafIdRef.current != null) {
        cancelAnimationFrame(rafIdRef.current);
        rafIdRef.current = null;
      }
    };
  }, []);

  const graphics = annotation.graphics ?? [];

  return (
    <div className="h-full w-full flex items-center justify-center bg-[var(--surface)] relative">
      <AnnotationGraphicsSvg
        annotation={annotation}
        size={{ width: 900, height: 700 }}
        selectedGraphicPath={selectedGraphicPath}
        className="block h-full w-full"
      />
      <svg
        ref={svgRef}
        width="100%"
        height="100%"
        viewBox="0 0 900 700"
        className="absolute inset-0 block cursor-crosshair"
        onPointerDown={(event) => {
          const svgElement = svgRef.current;
          if (!svgElement) return;
          const modelPoint = clientToModelPoint(event, svgElement, annotation);
          const snappedPoint = snapToGrid(modelPoint, snapOptions);
          const hitPath = findDeepestGraphicPath(graphics, snappedPoint);
          const additive = event.shiftKey || event.metaKey || event.ctrlKey;
          if (!hitPath && !additive) {
            onSelectGraphic(null, false);
          } else if (hitPath) {
            onSelectGraphic(hitPath, additive);
          }
          if (!readOnly && hitPath) {
            const current = getGraphicAtPath(graphics, hitPath);
            if (current?.layerLocked) {
              return;
            }
            if (current) {
              dragWorkingRef.current = cloneGraphicItem(current);
              pendingDeltaRef.current = { x: 0, y: 0 };
              dragStateRef.current = { path: hitPath, lastPoint: snappedPoint };
            }
          }
        }}
        onPointerMove={(event) => {
          if (readOnly || !dragStateRef.current || !svgRef.current) return;
          const nextPoint = clientToModelPoint(event, svgRef.current, annotation);
          const snappedPoint = snapToGrid(nextPoint, snapOptions);
          const delta = {
            x: snappedPoint.x - dragStateRef.current.lastPoint.x,
            y: snappedPoint.y - dragStateRef.current.lastPoint.y,
          };
          dragStateRef.current = { ...dragStateRef.current, lastPoint: snappedPoint };
          pendingDeltaRef.current = {
            x: pendingDeltaRef.current.x + delta.x,
            y: pendingDeltaRef.current.y + delta.y,
          };
          scheduleDragFlush();
        }}
      >
        {showGrid ? (
          <defs>
            <pattern
              id={gridPatternId}
              width={gridSize}
              height={gridSize}
              patternUnits="userSpaceOnUse"
            >
              <path
                d={`M ${gridSize} 0 L 0 0 0 ${gridSize}`}
                fill="none"
                stroke="rgba(128,128,128,0.22)"
                strokeWidth="0.5"
              />
            </pattern>
          </defs>
        ) : null}
        <rect
          x="0"
          y="0"
          width="900"
          height="700"
          fill={showGrid ? `url(#${gridPatternId})` : "transparent"}
        />
      </svg>
    </div>
  );
}
