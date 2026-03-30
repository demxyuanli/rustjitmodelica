import { useEffect, useId, useMemo, useRef, type PointerEvent as ReactPointerEvent } from "react";
import type {
  AnnotationPoint,
  GraphicEditHandle,
  GraphicItem,
  IconDiagramAnnotation,
} from "./diagramGraphicTypes";
import {
  AnnotationGraphicsSvg,
  applyGraphicEditHandleDrag,
  coordToSvg,
  findDeepestGraphicPath,
  getEditHandleHitTolerance,
  getGraphicAtPath,
  hitGraphicEditHandle,
  listGraphicEditHandlesInWorld,
  svgToCoord,
  translateGraphicItem,
} from "./DiagramSvgRenderer";
import { snapToGrid, type GridOptions } from "../utils/gridSnap";

const ICON_EDIT_SVG_W = 900;
const ICON_EDIT_SVG_H = 700;

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

type ActiveDrag =
  | null
  | { mode: "translate"; path: number[]; lastPoint: AnnotationPoint }
  | { mode: "handle"; path: number[]; handle: GraphicEditHandle; base: GraphicItem };

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
  const activeDragRef = useRef<ActiveDrag>(null);
  const dragWorkingRef = useRef<GraphicItem | null>(null);
  const pendingDeltaRef = useRef<AnnotationPoint>({ x: 0, y: 0 });
  const rafIdRef = useRef<number | null>(null);
  const handleLatestWorldRef = useRef<AnnotationPoint | null>(null);
  const handleRafRef = useRef<number | null>(null);
  const capturedPointerIdRef = useRef<number | null>(null);
  const onUpdateGraphicRef = useRef(onUpdateGraphic);
  onUpdateGraphicRef.current = onUpdateGraphic;
  const gridPatternId = useId().replace(/[^a-zA-Z0-9_-]/g, "_");

  const snapOptions = useMemo<GridOptions>(
    () => ({
      enabled: gridEnabled,
      gridSize,
      snapTolerance: gridSize / 2,
    }),
    [gridEnabled, gridSize],
  );

  const flushPendingDrag = () => {
    if (rafIdRef.current != null) {
      cancelAnimationFrame(rafIdRef.current);
      rafIdRef.current = null;
    }
    const st = activeDragRef.current;
    if (!st || st.mode !== "translate") {
      pendingDeltaRef.current = { x: 0, y: 0 };
      return;
    }
    const path = st.path;
    const d = pendingDeltaRef.current;
    if (path.length === 0 || (d.x === 0 && d.y === 0)) {
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
      const st = activeDragRef.current;
      if (!st || st.mode !== "translate") {
        pendingDeltaRef.current = { x: 0, y: 0 };
        return;
      }
      const path = st.path;
      const d = pendingDeltaRef.current;
      if (path.length === 0 || (d.x === 0 && d.y === 0)) {
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

  const scheduleHandleFlush = () => {
    if (handleRafRef.current != null) return;
    handleRafRef.current = requestAnimationFrame(() => {
      handleRafRef.current = null;
      const st = activeDragRef.current;
      if (!st || st.mode !== "handle") return;
      const w = handleLatestWorldRef.current;
      if (!w) return;
      const next = applyGraphicEditHandleDrag(structuredClone(st.base), st.handle, w);
      onUpdateGraphicRef.current(st.path, next);
    });
  };

  const releaseCaptureSafe = (pointerId: number) => {
    const el = svgRef.current;
    if (!el) return;
    try {
      if (el.hasPointerCapture(pointerId)) el.releasePointerCapture(pointerId);
    } catch {
      /* ignore */
    }
    if (capturedPointerIdRef.current === pointerId) capturedPointerIdRef.current = null;
  };

  useEffect(() => {
    const onWindowPointerUp = (ev: PointerEvent) => {
      if (handleRafRef.current != null) {
        cancelAnimationFrame(handleRafRef.current);
        handleRafRef.current = null;
      }
      const st = activeDragRef.current;
      if (st?.mode === "handle" && handleLatestWorldRef.current) {
        const w = handleLatestWorldRef.current;
        const next = applyGraphicEditHandleDrag(structuredClone(st.base), st.handle, w);
        onUpdateGraphicRef.current(st.path, next);
      }
      flushPendingDrag();
      activeDragRef.current = null;
      dragWorkingRef.current = null;
      handleLatestWorldRef.current = null;
      pendingDeltaRef.current = { x: 0, y: 0 };
      releaseCaptureSafe(ev.pointerId);
    };
    window.addEventListener("pointerup", onWindowPointerUp);
    window.addEventListener("pointercancel", onWindowPointerUp);
    return () => {
      window.removeEventListener("pointerup", onWindowPointerUp);
      window.removeEventListener("pointercancel", onWindowPointerUp);
      if (rafIdRef.current != null) {
        cancelAnimationFrame(rafIdRef.current);
        rafIdRef.current = null;
      }
      if (handleRafRef.current != null) {
        cancelAnimationFrame(handleRafRef.current);
        handleRafRef.current = null;
      }
    };
  }, []);

  const graphics = annotation.graphics ?? [];

  const handleOverlay = useMemo(() => {
    if (!selectedGraphicPath?.length) return null;
    const sel = getGraphicAtPath(graphics, selectedGraphicPath);
    if (!sel || sel.type === "Group" || sel.layerHidden) return null;
    const list = listGraphicEditHandlesInWorld(sel);
    const cs = annotation.coordinateSystem;
    return list.map(({ handle, world }) => {
      const p = coordToSvg(world, cs, ICON_EDIT_SVG_W, ICON_EDIT_SVG_H);
      const key = handle.kind === "extent-corner" ? `c-${handle.cornerIndex}` : `p-${handle.pointIndex}`;
      return (
        <circle
          key={key}
          cx={p.x}
          cy={p.y}
          r={6}
          fill="#0ea5e9"
          stroke="#fff"
          strokeWidth={1.5}
          style={{ pointerEvents: "none" }}
        />
      );
    });
  }, [annotation.coordinateSystem, graphics, selectedGraphicPath]);

  return (
    <div className="h-full w-full flex items-center justify-center bg-[var(--surface)] relative">
      <AnnotationGraphicsSvg
        annotation={annotation}
        size={{ width: ICON_EDIT_SVG_W, height: ICON_EDIT_SVG_H }}
        selectedGraphicPath={selectedGraphicPath}
        className="block h-full w-full"
      />
      <svg
        ref={svgRef}
        width="100%"
        height="100%"
        viewBox={`0 0 ${ICON_EDIT_SVG_W} ${ICON_EDIT_SVG_H}`}
        className="absolute inset-0 block cursor-crosshair"
        onPointerDown={(event) => {
          if (event.button !== 0) return;
          const svgElement = svgRef.current;
          if (!svgElement) return;
          const modelPoint = clientToModelPoint(event, svgElement, annotation);
          const snappedPoint = snapToGrid(modelPoint, snapOptions);

          if (!readOnly && selectedGraphicPath?.length) {
            const currentSel = getGraphicAtPath(graphics, selectedGraphicPath);
            if (
              currentSel &&
              !currentSel.layerLocked &&
              currentSel.type !== "Group" &&
              !currentSel.layerHidden
            ) {
              const tol = getEditHandleHitTolerance(annotation.coordinateSystem);
              const h = hitGraphicEditHandle(currentSel, snappedPoint, tol);
              if (h) {
                svgElement.setPointerCapture(event.pointerId);
                capturedPointerIdRef.current = event.pointerId;
                activeDragRef.current = {
                  mode: "handle",
                  path: selectedGraphicPath,
                  handle: h,
                  base: cloneGraphicItem(currentSel),
                };
                handleLatestWorldRef.current = snappedPoint;
                scheduleHandleFlush();
                return;
              }
            }
          }

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
              svgElement.setPointerCapture(event.pointerId);
              capturedPointerIdRef.current = event.pointerId;
              dragWorkingRef.current = cloneGraphicItem(current);
              pendingDeltaRef.current = { x: 0, y: 0 };
              activeDragRef.current = { mode: "translate", path: hitPath, lastPoint: snappedPoint };
            }
          }
        }}
        onPointerMove={(event) => {
          if (readOnly || !activeDragRef.current || !svgRef.current) return;
          const nextPoint = clientToModelPoint(event, svgRef.current, annotation);
          const snappedPoint = snapToGrid(nextPoint, snapOptions);
          const st = activeDragRef.current;
          if (st.mode === "handle") {
            handleLatestWorldRef.current = snappedPoint;
            scheduleHandleFlush();
            return;
          }
          if (st.mode !== "translate") return;
          const delta = {
            x: snappedPoint.x - st.lastPoint.x,
            y: snappedPoint.y - st.lastPoint.y,
          };
          activeDragRef.current = { ...st, lastPoint: snappedPoint };
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
          width={ICON_EDIT_SVG_W}
          height={ICON_EDIT_SVG_H}
          fill={showGrid ? `url(#${gridPatternId})` : "transparent"}
        />
        {handleOverlay}
      </svg>
    </div>
  );
}
