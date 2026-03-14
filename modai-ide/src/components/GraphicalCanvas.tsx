import type { DragEvent, ReactNode } from "react";
import { AnnotationGraphicsSvg, type GraphicItem, type IconDiagramAnnotation } from "./DiagramSvgRenderer";
import { LibrariesBrowser } from "./LibrariesBrowser";
import { createDefaultGraphic, ModelicaPropertyPanel } from "./ModelicaPropertyPanel";
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

export interface GraphicalMessage {
  severity: "info" | "warning" | "error";
  text: string;
}

interface GraphicalCanvasProps {
  modelName: string;
  projectDir: string | null;
  mode: "icon" | "diagram";
  readOnly: boolean;
  annotation?: IconDiagramAnnotation;
  graphics: GraphicItem[];
  selectedGraphicIndex: number;
  selectedComponent: SelectedComponent | null;
  conflictPending: boolean;
  messages: GraphicalMessage[];
  onRefreshDiagram?: () => void;
  onSelectGraphic: (index: number) => void;
  onUpdateGraphic: (index: number, next: GraphicItem) => void;
  onAddGraphic: (graphic: GraphicItem) => void;
  onDeleteGraphic: (index: number) => void;
  onUpdateParam: (name: string, value: string) => void;
  onUpdatePlacement: (patch: { x?: number; y?: number; rotation?: number }) => void;
  onOpenType?: (typeName: string, libraryId?: string) => void;
  libraryRefreshToken?: number;
  onDrop?: (event: DragEvent) => void;
  onDragOver?: (event: DragEvent) => void;
  children: ReactNode;
}

function severityClass(severity: GraphicalMessage["severity"]) {
  switch (severity) {
    case "error":
      return "text-red-300 border-red-500/30 bg-red-500/10";
    case "warning":
      return "text-amber-300 border-amber-500/30 bg-amber-500/10";
    default:
      return "text-sky-300 border-sky-500/30 bg-sky-500/10";
  }
}

const ICON_TOOLBAR_ITEMS: Array<{ kind: GraphicItem["type"]; label: string }> = [
  { kind: "Rectangle", label: "Rect" },
  { kind: "Ellipse", label: "Ellipse" },
  { kind: "Line", label: "Line" },
  { kind: "Polygon", label: "Polygon" },
  { kind: "Text", label: "Text" },
];

export function GraphicalCanvas({
  modelName,
  projectDir,
  mode,
  readOnly,
  annotation,
  graphics,
  selectedGraphicIndex,
  selectedComponent,
  conflictPending,
  messages,
  onRefreshDiagram,
  onSelectGraphic,
  onUpdateGraphic,
  onAddGraphic,
  onDeleteGraphic,
  onUpdateParam,
  onUpdatePlacement,
  onOpenType,
  libraryRefreshToken = 0,
  onDrop,
  onDragOver,
  children,
}: GraphicalCanvasProps) {
  const placement = selectedComponent?.placement?.transformation;
  const origin = placement?.origin ?? { x: 0, y: 0 };
  const rotation = placement?.rotation ?? 0;

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
          <LibrariesBrowser
            projectDir={projectDir}
            readOnly={readOnly}
            onOpenType={onOpenType}
            libraryRefreshToken={libraryRefreshToken}
          />
        )}
        <div className="flex-1 min-w-0 flex flex-col relative" onDrop={onDrop} onDragOver={onDragOver}>
          {!readOnly && (mode === "icon" || mode === "diagram") && (
            <div className="absolute left-1/2 top-2 z-20 flex -translate-x-1/2 items-center gap-1 rounded-lg border border-[var(--border)] bg-[var(--bg-elevated)]/95 px-2 py-1 shadow-lg backdrop-blur">
              {ICON_TOOLBAR_ITEMS.map((item) => (
                <button
                  key={item.kind}
                  type="button"
                  className="rounded border border-[var(--border)] bg-[var(--surface)] px-2 py-1 text-[11px] text-[var(--text)] hover:bg-white/10"
                  onClick={() => onAddGraphic(createDefaultGraphic(item.kind))}
                >
                  {item.label}
                </button>
              ))}
            </div>
          )}
          {annotation && annotation.graphics.length > 0 && mode === "diagram" && (
            <div className="absolute inset-0 z-0 opacity-40 flex items-center justify-center overflow-hidden">
              <AnnotationGraphicsSvg annotation={annotation} size={{ width: 900, height: 700 }} />
            </div>
          )}
          <div className="absolute top-2 left-3 z-10 text-xs text-[var(--text-muted)] pointer-events-none">
            {modelName}
          </div>
          {((mode === "icon" && selectedGraphicIndex >= 0) || (mode === "diagram" && (selectedGraphicIndex >= 0 || selectedComponent != null))) && (
            <div className="absolute right-3 top-14 z-20">
              <ModelicaPropertyPanel
                projectDir={projectDir}
                mode={mode}
                presentation="floating"
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
          )}
          <div className="relative z-10 flex-1 min-h-0">{children}</div>
          <div className="shrink-0 border-t border-[var(--border)] bg-[var(--bg-elevated)] px-2 py-1.5">
            <div className="flex items-center justify-between gap-2 flex-wrap">
              <div className="flex items-center gap-3 flex-wrap">
                <div className="text-[10px] uppercase tracking-wide text-[var(--text-muted)]">
                  {t("messagesBrowser")}
                </div>
                {mode === "diagram" && selectedComponent && (
                  <div className="flex items-center gap-2 flex-wrap text-[10px] text-[var(--text-muted)]">
                    <span className="text-[var(--text)] font-medium">{selectedComponent.name}</span>
                    <span>{selectedComponent.typeName}</span>
                    <label className="flex items-center gap-1">
                      <span>X</span>
                      <input
                        type="number"
                        value={origin.x}
                        onChange={(e) => onUpdatePlacement({ x: Number(e.target.value) })}
                        className="w-16 rounded bg-[var(--surface)] border border-[var(--border)] px-1 py-0.5 text-[var(--text)]"
                      />
                    </label>
                    <label className="flex items-center gap-1">
                      <span>Y</span>
                      <input
                        type="number"
                        value={origin.y}
                        onChange={(e) => onUpdatePlacement({ y: Number(e.target.value) })}
                        className="w-16 rounded bg-[var(--surface)] border border-[var(--border)] px-1 py-0.5 text-[var(--text)]"
                      />
                    </label>
                    <label className="flex items-center gap-1">
                      <span>Rotation</span>
                      <input
                        type="number"
                        value={rotation}
                        onChange={(e) => onUpdatePlacement({ rotation: Number(e.target.value) })}
                        className="w-16 rounded bg-[var(--surface)] border border-[var(--border)] px-1 py-0.5 text-[var(--text)]"
                      />
                    </label>
                  </div>
                )}
              </div>
              <div className="text-[10px] text-[var(--text-muted)]">
                {messages.length}
              </div>
            </div>
            <div className="mt-1 max-h-24 overflow-auto space-y-1 text-xs">
              {messages.length === 0 ? (
                <div className="text-[var(--text-muted)]">{t("noIssues")}</div>
              ) : (
                messages.map((message, index) => (
                  <div
                    key={`${message.severity}:${index}`}
                    className={`rounded border px-2 py-1 ${severityClass(message.severity)}`}
                  >
                    {message.text}
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
