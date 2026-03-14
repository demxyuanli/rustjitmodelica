import { useEffect, useMemo, useRef, type PointerEvent as ReactPointerEvent } from "react";
import {
  AnnotationGraphicsSvg,
  findGraphicAtPoint,
  svgToCoord,
  translateGraphicItem,
  type AnnotationPoint,
  type GraphicItem,
  type IconDiagramAnnotation,
} from "./DiagramSvgRenderer";

interface IconEditorViewProps {
  annotation: IconDiagramAnnotation;
  selectedGraphicIndex: number;
  readOnly: boolean;
  onSelectGraphic: (index: number) => void;
  onUpdateGraphic: (index: number, next: GraphicItem) => void;
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
  selectedGraphicIndex,
  readOnly,
  onSelectGraphic,
  onUpdateGraphic,
}: IconEditorViewProps) {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const dragStateRef = useRef<{ index: number; lastPoint: AnnotationPoint } | null>(null);

  const safeAnnotation = useMemo<IconDiagramAnnotation>(
    () => ({
      coordinateSystem: annotation.coordinateSystem,
      graphics: annotation.graphics ?? [],
    }),
    [annotation],
  );

  useEffect(() => {
    const onWindowPointerUp = () => {
      dragStateRef.current = null;
    };
    window.addEventListener("pointerup", onWindowPointerUp);
    return () => {
      window.removeEventListener("pointerup", onWindowPointerUp);
    };
  }, []);

  return (
    <div className="h-full w-full flex items-center justify-center bg-[var(--surface)] relative">
      <AnnotationGraphicsSvg
        annotation={safeAnnotation}
        size={{ width: 900, height: 700 }}
        selectedGraphicIndex={selectedGraphicIndex}
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
          const modelPoint = clientToModelPoint(event, svgElement, safeAnnotation);
          const hitIndex = findGraphicAtPoint(safeAnnotation.graphics, modelPoint);
          onSelectGraphic(hitIndex);
          if (!readOnly && hitIndex >= 0) {
            dragStateRef.current = { index: hitIndex, lastPoint: modelPoint };
          }
        }}
        onPointerMove={(event) => {
          if (readOnly || !dragStateRef.current || !svgRef.current) return;
          const nextPoint = clientToModelPoint(event, svgRef.current, safeAnnotation);
          const delta = {
            x: nextPoint.x - dragStateRef.current.lastPoint.x,
            y: nextPoint.y - dragStateRef.current.lastPoint.y,
          };
          const current = safeAnnotation.graphics[dragStateRef.current.index];
          if (!current) return;
          onUpdateGraphic(dragStateRef.current.index, translateGraphicItem(current, delta));
          dragStateRef.current = { ...dragStateRef.current, lastPoint: nextPoint };
        }}
      >
        <rect x="0" y="0" width="900" height="700" fill="transparent" />
      </svg>
    </div>
  );
}
