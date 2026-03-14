import { useEffect, useMemo, useState } from "react";
import { getComponentTypeDetails } from "../api/tauri";
import type { ComponentTypeInfo } from "../types";
import { t } from "../i18n";
import type {
  AnnotationColor,
  GraphicEllipse,
  GraphicItem,
  GraphicLine,
  GraphicPolygon,
  GraphicRectangle,
  GraphicText,
  AnnotationPoint,
} from "./DiagramSvgRenderer";

interface PlacementData {
  transformation?: {
    origin?: AnnotationPoint;
    extent?: { p1: AnnotationPoint; p2: AnnotationPoint };
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

function groupKey(tab?: string, group?: string) {
  return `${tab || "General"}::${group || "Parameters"}`;
}

function cloneGraphic(item: GraphicItem): GraphicItem {
  return JSON.parse(JSON.stringify(item)) as GraphicItem;
}

type GraphicEditorField =
  | { type: "text"; label: string; value: string; onChange: (value: string) => void }
  | { type: "number"; label: string; value: number; onChange: (value: number) => void }
  | { type: "color"; label: string; value?: AnnotationColor; onChange: (value: AnnotationColor) => void };

function colorToHex(color?: AnnotationColor) {
  if (!color) return "#808080";
  return `#${[color.r, color.g, color.b]
    .map((channel) => Math.max(0, Math.min(255, channel)).toString(16).padStart(2, "0"))
    .join("")}`;
}

function hexToColor(hex: string): AnnotationColor {
  const match = /^#?([0-9a-f]{6})$/i.exec(hex);
  if (!match) return { r: 128, g: 128, b: 128 };
  const value = match[1];
  return {
    r: Number.parseInt(value.slice(0, 2), 16),
    g: Number.parseInt(value.slice(2, 4), 16),
    b: Number.parseInt(value.slice(4, 6), 16),
  };
}

export function createDefaultGraphic(kind: GraphicItem["type"]): GraphicItem {
  switch (kind) {
    case "Rectangle":
      return {
        type: "Rectangle",
        extent: { p1: { x: -60, y: 40 }, p2: { x: 60, y: -40 } },
        lineColor: { r: 0, g: 0, b: 255 },
      } satisfies GraphicRectangle;
    case "Ellipse":
      return {
        type: "Ellipse",
        extent: { p1: { x: -50, y: 50 }, p2: { x: 50, y: -50 } },
        lineColor: { r: 191, g: 0, b: 0 },
      } satisfies GraphicEllipse;
    case "Line":
      return {
        type: "Line",
        points: [{ x: -80, y: 0 }, { x: 80, y: 0 }],
        color: { r: 0, g: 127, b: 0 },
      } satisfies GraphicLine;
    case "Polygon":
      return {
        type: "Polygon",
        points: [{ x: -60, y: -40 }, { x: 0, y: 60 }, { x: 60, y: -40 }],
        lineColor: { r: 85, g: 85, b: 85 },
      } satisfies GraphicPolygon;
    case "Text":
      return {
        type: "Text",
        extent: { p1: { x: -100, y: 90 }, p2: { x: 100, y: 50 } },
        textString: "%name",
        textColor: { r: 0, g: 0, b: 255 },
      } satisfies GraphicText;
    case "Bitmap":
      return {
        type: "Bitmap",
        extent: { p1: { x: -50, y: 50 }, p2: { x: 50, y: -50 } },
        fileName: "",
      };
    default:
      return {
        type: "Rectangle",
        extent: { p1: { x: -40, y: 40 }, p2: { x: 40, y: -40 } },
      } satisfies GraphicRectangle;
  }
}

interface ModelicaPropertyPanelProps {
  projectDir: string | null;
  mode: "icon" | "diagram";
  presentation?: "sidebar" | "floating";
  selectedComponent: SelectedComponent | null;
  graphics: GraphicItem[];
  selectedGraphicIndex: number;
  onSelectGraphic: (index: number) => void;
  onUpdateGraphic: (index: number, next: GraphicItem) => void;
  onAddGraphic: (graphic: GraphicItem) => void;
  onDeleteGraphic: (index: number) => void;
  onUpdateParam: (name: string, value: string) => void;
  onUpdatePlacement: (patch: { x?: number; y?: number; rotation?: number }) => void;
}

function updateGraphicField<T extends GraphicItem>(
  graphic: GraphicItem,
  cast: (item: GraphicItem) => T,
  mutator: (next: T) => void,
) {
  const next = cloneGraphic(graphic);
  mutator(cast(next));
  return next;
}

function renderField(field: GraphicEditorField, key: string) {
  return (
    <label key={key} className="block">
      <div className="mb-1 text-[var(--text-muted)]">{field.label}</div>
      {field.type === "text" && (
        <input
          type="text"
          value={field.value}
          onChange={(event) => field.onChange(event.target.value)}
          className="w-full rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1"
        />
      )}
      {field.type === "number" && (
        <input
          type="number"
          value={field.value}
          onChange={(event) => field.onChange(Number(event.target.value))}
          className="w-full rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1"
        />
      )}
      {field.type === "color" && (
        <input
          type="color"
          value={colorToHex(field.value)}
          onChange={(event) => field.onChange(hexToColor(event.target.value))}
          className="h-8 w-full rounded bg-[var(--surface)] border border-[var(--border)] px-1"
        />
      )}
    </label>
  );
}

function graphicFields(
  graphic: GraphicItem,
  onChange: (next: GraphicItem) => void,
): GraphicEditorField[] {
  const fields: GraphicEditorField[] = [];

  if ("textString" in graphic) {
    fields.push({
      type: "text",
      label: "Text",
      value: graphic.textString ?? "",
      onChange: (value) =>
        onChange(
          updateGraphicField(graphic, (item) => item as GraphicText, (next) => {
            next.textString = value;
          }),
        ),
    });
  }

  if ("rotation" in graphic) {
    fields.push({
      type: "number",
      label: "Rotation",
      value: graphic.rotation ?? 0,
      onChange: (value) =>
        onChange(
          updateGraphicField(graphic, (item) => item as GraphicItem, (next) => {
            next.rotation = value;
          }),
        ),
    });
  }

  if ("lineThickness" in graphic) {
    fields.push({
      type: "number",
      label: "Line thickness",
      value: graphic.lineThickness ?? 1,
      onChange: (value) =>
        onChange(
          updateGraphicField(graphic, (item) => item as GraphicRectangle | GraphicEllipse | GraphicPolygon, (next) => {
            next.lineThickness = value;
          }),
        ),
    });
  }

  if ("thickness" in graphic) {
    fields.push({
      type: "number",
      label: "Thickness",
      value: graphic.thickness ?? 1,
      onChange: (value) =>
        onChange(
          updateGraphicField(graphic, (item) => item as GraphicLine, (next) => {
            next.thickness = value;
          }),
        ),
    });
  }

  if ("lineColor" in graphic) {
    fields.push({
      type: "color",
      label: "Line color",
      value: graphic.lineColor,
      onChange: (value) =>
        onChange(
          updateGraphicField(graphic, (item) => item as GraphicRectangle | GraphicEllipse | GraphicPolygon | GraphicText, (next) => {
            next.lineColor = value;
          }),
        ),
    });
  }

  if ("fillColor" in graphic) {
    fields.push({
      type: "color",
      label: "Fill color",
      value: graphic.fillColor,
      onChange: (value) =>
        onChange(
          updateGraphicField(graphic, (item) => item as GraphicRectangle | GraphicEllipse | GraphicPolygon | GraphicText, (next) => {
            next.fillColor = value;
          }),
        ),
    });
  }

  if ("textColor" in graphic) {
    fields.push({
      type: "color",
      label: "Text color",
      value: graphic.textColor,
      onChange: (value) =>
        onChange(
          updateGraphicField(graphic, (item) => item as GraphicText, (next) => {
            next.textColor = value;
          }),
        ),
    });
  }

  if ("color" in graphic) {
    fields.push({
      type: "color",
      label: "Color",
      value: graphic.color,
      onChange: (value) =>
        onChange(
          updateGraphicField(graphic, (item) => item as GraphicLine, (next) => {
            next.color = value;
          }),
        ),
    });
  }

  return fields;
}

export function ModelicaPropertyPanel({
  projectDir,
  mode,
  presentation = "sidebar",
  selectedComponent,
  graphics,
  selectedGraphicIndex,
  onSelectGraphic,
  onUpdateGraphic,
  onAddGraphic,
  onDeleteGraphic,
  onUpdateParam,
  onUpdatePlacement,
}: ModelicaPropertyPanelProps) {
  const [typeInfo, setTypeInfo] = useState<ComponentTypeInfo | null>(null);

  useEffect(() => {
    if (!projectDir || !selectedComponent?.typeName) {
      setTypeInfo(null);
      return;
    }
    let cancelled = false;
    getComponentTypeDetails(projectDir, selectedComponent.typeName, selectedComponent.libraryId)
      .then((info) => {
        if (!cancelled) {
          setTypeInfo(info);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setTypeInfo(null);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [projectDir, selectedComponent?.typeName, selectedComponent?.libraryId]);

  const groupedParams = useMemo(() => {
    if (!typeInfo) {
      return [];
    }
    const groups = new Map<string, typeof typeInfo.parameters>();
    for (const parameter of typeInfo.parameters) {
      const key = groupKey(parameter.dialog?.tab, parameter.dialog?.group);
      const bucket = groups.get(key) ?? [];
      bucket.push(parameter);
      groups.set(key, bucket);
    }
    return Array.from(groups.entries()).map(([key, value]) => {
      const [tab, group] = key.split("::");
      return { tab, group, parameters: value };
    });
  }, [typeInfo]);

  const currentGraphic = selectedGraphicIndex >= 0 ? graphics[selectedGraphicIndex] : null;
  const placement = selectedComponent?.placement?.transformation;
  const origin = placement?.origin ?? { x: 0, y: 0 };
  const rotation = placement?.rotation ?? 0;
  const componentParams = new Map((selectedComponent?.params ?? []).map((item) => [item.name, item.value]));
  const currentGraphicFields = currentGraphic
    ? graphicFields(currentGraphic, (next) => onUpdateGraphic(selectedGraphicIndex, next))
    : [];
  const showGraphicLibrary = presentation === "sidebar";
  const showComponentSection = Boolean(selectedComponent) && (mode === "icon" || groupedParams.length > 0);
  const showGraphicsSection = mode === "icon" || currentGraphic != null || presentation === "sidebar";
  const hasVisibleDiagramDetails = showComponentSection || currentGraphic != null;
  const containerClass =
    presentation === "floating"
      ? mode === "diagram"
        ? "w-80 max-h-[calc(100vh-10rem)] rounded-lg border border-[var(--border)] bg-[var(--bg-elevated)] shadow-2xl flex flex-col overflow-hidden"
        : "w-80 max-h-[70vh] rounded-lg border border-[var(--border)] bg-[var(--bg-elevated)] shadow-2xl flex flex-col overflow-hidden"
      : "w-72 shrink-0 border-l border-[var(--border)] bg-[var(--bg-elevated)] flex flex-col min-h-0";

  if (mode === "diagram" && presentation === "floating" && !hasVisibleDiagramDetails) {
    return null;
  }

  return (
    <div className={containerClass}>
      <div className="px-3 py-2 border-b border-[var(--border)]">
        <div className="text-xs font-medium text-[var(--text-muted)]">
          {mode === "icon" ? t("iconProperties") : t("diagramProperties")}
        </div>
      </div>
      <div className="flex-1 overflow-auto p-3 space-y-4 text-xs">
        {showComponentSection && selectedComponent && (
          <section className="space-y-2">
            <div className="font-medium text-[var(--text)]">{selectedComponent.name}</div>
            <div className="text-[var(--text-muted)]">{selectedComponent.typeName}</div>
            {mode === "icon" && (
              <div className="grid grid-cols-3 gap-2 items-center">
                <span>X</span>
                <input
                  type="number"
                  value={origin.x}
                  onChange={(e) => onUpdatePlacement({ x: Number(e.target.value) })}
                  className="col-span-2 rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1"
                />
                <span>Y</span>
                <input
                  type="number"
                  value={origin.y}
                  onChange={(e) => onUpdatePlacement({ y: Number(e.target.value) })}
                  className="col-span-2 rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1"
                />
                <span>Rotation</span>
                <input
                  type="number"
                  value={rotation}
                  onChange={(e) => onUpdatePlacement({ rotation: Number(e.target.value) })}
                  className="col-span-2 rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1"
                />
              </div>
            )}
            {groupedParams.map((group) => (
              <div key={`${group.tab}:${group.group}`} className="border border-[var(--border)] rounded">
                <div className="px-2 py-1 border-b border-[var(--border)] text-[var(--text-muted)]">
                  {group.tab} / {group.group}
                </div>
                <div className="p-2 space-y-2">
                  {group.parameters.map((parameter) => (
                    <label key={parameter.name} className="block">
                      <div className="mb-1 text-[var(--text-muted)]">
                        {parameter.name}
                        <span className="ml-1 opacity-70">({parameter.typeName})</span>
                      </div>
                      <input
                        type="text"
                        value={componentParams.get(parameter.name) ?? parameter.defaultValue ?? ""}
                        onChange={(e) => onUpdateParam(parameter.name, e.target.value)}
                        className="w-full rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1 text-[var(--text)]"
                      />
                    </label>
                  ))}
                </div>
              </div>
            ))}
          </section>
        )}

        {showGraphicsSection && (
          <section className="space-y-2">
            <div className="flex items-center justify-between">
              <div className="font-medium text-[var(--text)]">
                {mode === "icon" ? t("iconGraphics") : t("diagramGraphics")}
              </div>
              <div className="text-[var(--text-muted)]">{graphics.length}</div>
            </div>
            {showGraphicLibrary && (
              <div className="grid grid-cols-2 gap-1">
                {(["Rectangle", "Ellipse", "Line", "Polygon", "Text"] as GraphicItem["type"][]).map((kind) => (
                  <button
                    key={kind}
                    type="button"
                    className="rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1 hover:bg-white/10"
                    onClick={() => onAddGraphic(createDefaultGraphic(kind))}
                  >
                    + {kind}
                  </button>
                ))}
              </div>
            )}
            <div className="space-y-1">
              {graphics.map((graphic, index) => (
                <button
                  key={`${graphic.type}:${index}`}
                  type="button"
                  className={`w-full text-left rounded px-2 py-1 border ${
                    selectedGraphicIndex === index
                      ? "border-primary bg-primary/10"
                      : "border-[var(--border)] bg-[var(--surface)]"
                  }`}
                  onClick={() => onSelectGraphic(index)}
                >
                  {graphic.type} #{index + 1}
                </button>
              ))}
            </div>
            {currentGraphic && (
              <div className="border border-[var(--border)] rounded p-2 space-y-2">
                <div className="flex items-center justify-between">
                  <span className="font-medium">{currentGraphic.type}</span>
                  <button
                    type="button"
                    className="rounded bg-red-600/20 px-2 py-1 text-red-300"
                    onClick={() => onDeleteGraphic(selectedGraphicIndex)}
                  >
                    {t("deleteTest")}
                  </button>
                </div>
                <div className="grid grid-cols-1 gap-2">
                  {currentGraphicFields.map((field, index) => renderField(field, `field-${index}`))}
                </div>
                {"extent" in currentGraphic && currentGraphic.extent && (
                  <div className="grid grid-cols-2 gap-2">
                    {(["p1", "p2"] as const).flatMap((corner) => ([
                      <label key={`${corner}.x`} className="block">
                        <div className="mb-1 text-[var(--text-muted)]">{corner}.x</div>
                        <input
                          type="number"
                          value={currentGraphic.extent?.[corner].x ?? 0}
                          onChange={(e) => {
                            const next = cloneGraphic(currentGraphic) as GraphicRectangle;
                            if (next.extent) {
                              next.extent[corner].x = Number(e.target.value);
                            }
                            onUpdateGraphic(selectedGraphicIndex, next);
                          }}
                          className="w-full rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1"
                        />
                      </label>,
                      <label key={`${corner}.y`} className="block">
                        <div className="mb-1 text-[var(--text-muted)]">{corner}.y</div>
                        <input
                          type="number"
                          value={currentGraphic.extent?.[corner].y ?? 0}
                          onChange={(e) => {
                            const next = cloneGraphic(currentGraphic) as GraphicRectangle;
                            if (next.extent) {
                              next.extent[corner].y = Number(e.target.value);
                            }
                            onUpdateGraphic(selectedGraphicIndex, next);
                          }}
                          className="w-full rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1"
                        />
                      </label>,
                    ]))}
                  </div>
                )}
                {"points" in currentGraphic && currentGraphic.points.length > 0 && (
                  <div className="space-y-2">
                    {currentGraphic.points.map((point, pointIndex) => (
                      <div key={`point-${pointIndex}`} className="grid grid-cols-2 gap-2">
                        <label className="block">
                          <div className="mb-1 text-[var(--text-muted)]">p{pointIndex}.x</div>
                          <input
                            type="number"
                            value={point.x}
                            onChange={(event) => {
                              const next = cloneGraphic(currentGraphic) as GraphicLine | GraphicPolygon;
                              next.points[pointIndex].x = Number(event.target.value);
                              onUpdateGraphic(selectedGraphicIndex, next);
                            }}
                            className="w-full rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1"
                          />
                        </label>
                        <label className="block">
                          <div className="mb-1 text-[var(--text-muted)]">p{pointIndex}.y</div>
                          <input
                            type="number"
                            value={point.y}
                            onChange={(event) => {
                              const next = cloneGraphic(currentGraphic) as GraphicLine | GraphicPolygon;
                              next.points[pointIndex].y = Number(event.target.value);
                              onUpdateGraphic(selectedGraphicIndex, next);
                            }}
                            className="w-full rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1"
                          />
                        </label>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}
          </section>
        )}
      </div>
    </div>
  );
}
