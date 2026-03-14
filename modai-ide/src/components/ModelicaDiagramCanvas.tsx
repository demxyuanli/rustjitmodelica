import type { ReactNode } from "react";
import { AnnotationGraphicsSvg, type GraphicItem, type IconDiagramAnnotation } from "./DiagramSvgRenderer";
import { ModelicaLibraryBrowser } from "./ModelicaLibraryBrowser";
import { ModelicaPropertyPanel } from "./ModelicaPropertyPanel";
import { t } from "../i18n";

interface PlacementData {
  transformation?: {
    origin?: { x: number; y: number };
    extent?: { p1: { x: number; y: number }; p2: { x: number; y: number } };
    rotation?: number;
  };
}

interface ParamValue {
  name: string;
  value: string;
}

interface SelectedComponent {
  name: string;
  typeName: string;
  libraryId?: string;
  params?: ParamValue[];
  placement?: PlacementData;
}

interface ModelicaDiagramCanvasProps {
  modelName: string;
  projectDir: string | null;
  mode: "icon" | "diagram";
  readOnly: boolean;
  annotation?: IconDiagramAnnotation;
  graphics: GraphicItem[];
  selectedGraphicIndex: number;
  selectedComponent: SelectedComponent | null;
  conflictPending: boolean;
  onRefreshDiagram?: () => void;
  onSelectGraphic: (index: number) => void;
  onUpdateGraphic: (index: number, next: GraphicItem) => void;
  onAddGraphic: (graphic: GraphicItem) => void;
  onDeleteGraphic: (index: number) => void;
  onUpdateParam: (name: string, value: string) => void;
  onUpdatePlacement: (patch: { x?: number; y?: number; rotation?: number }) => void;
  onOpenType?: (typeName: string, libraryId?: string) => void;
  onDrop?: (event: React.DragEvent) => void;
  onDragOver?: (event: React.DragEvent) => void;
  children: ReactNode;
}

export function ModelicaDiagramCanvas({
  modelName,
  projectDir,
  mode,
  readOnly,
  annotation,
  graphics,
  selectedGraphicIndex,
  selectedComponent,
  conflictPending,
  onRefreshDiagram,
  onSelectGraphic,
  onUpdateGraphic,
  onAddGraphic,
  onDeleteGraphic,
  onUpdateParam,
  onUpdatePlacement,
  onOpenType,
  onDrop,
  onDragOver,
  children,
}: ModelicaDiagramCanvasProps) {
  return (
    <div className="h-full w-full flex flex-col">
      {conflictPending && (
        <div
          className="shrink-0 flex items-center justify-between gap-2 px-3 py-2 bg-amber-500/20 border-b border-amber-500/40 text-[var(--text)]"
          role="alert"
        >
          <span className="text-sm">{t("diagramConflict")}</span>
          <button
            type="button"
            className="px-2 py-1 text-xs font-medium rounded bg-primary text-white hover:opacity-90"
            onClick={onRefreshDiagram}
          >
            {t("refreshDiagram")}
          </button>
        </div>
      )}
      <div className="flex-1 min-h-0 flex">
        {!readOnly && projectDir && mode === "diagram" && (
          <ModelicaLibraryBrowser projectDir={projectDir} readOnly={readOnly} onOpenType={onOpenType} />
        )}
        <div className="flex-1 min-w-0 relative" onDrop={onDrop} onDragOver={onDragOver}>
          {annotation && annotation.graphics.length > 0 && (
            <div className="absolute inset-0 z-0 opacity-60 flex items-center justify-center overflow-hidden">
              <AnnotationGraphicsSvg annotation={annotation} size={{ width: 900, height: 700 }} />
            </div>
          )}
          <div className="absolute top-2 left-3 z-10 text-xs text-[var(--text-muted)] pointer-events-none">
            {modelName}
          </div>
          <div className="relative z-10 h-full">{children}</div>
        </div>
        <ModelicaPropertyPanel
          projectDir={projectDir}
          mode={mode}
          selectedComponent={selectedComponent}
          graphics={graphics}
          selectedGraphicIndex={selectedGraphicIndex}
          onSelectGraphic={onSelectGraphic}
          onUpdateGraphic={onUpdateGraphic}
          onAddGraphic={onAddGraphic}
          onDeleteGraphic={onDeleteGraphic}
          onUpdateParam={onUpdateParam}
          onUpdatePlacement={onUpdatePlacement}
        />
      </div>
    </div>
  );
}
