import { useCallback, useEffect, useMemo, useState } from "react";
import { ZoomIn, ZoomOut, Maximize2, Maximize, GripVertical, Settings } from "lucide-react";
import { getComponentTypeDetails } from "../api/tauri";
import type { ComponentTypeInfo } from "../types";
import { t } from "../i18n";
import type {
  AnnotationColor,
  AnnotationPoint,
  GradientStop,
  GraphicBSpline,
  GraphicEllipse,
  GraphicItem,
  GraphicLine,
  GraphicPolygon,
  GraphicRectangle,
  GraphicText,
  LinearGradient,
  RadialGradient,
} from "./diagramGraphicTypes";
import { getGraphicAtPath } from "./DiagramSvgRenderer";
import { EquationGraphView } from "./EquationGraphView";
import { DependencyGraphModal } from "./DependencyGraphModal";
import type { JointPaperHandle } from "../utils/jointUtils";
import type { DependencyGraphBehavior } from "../utils/dependencyGraphBehavior";

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
  replaceable?: boolean;
  constrainedbyType?: string;
  condition?: string;
  visible?: boolean;
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
  | { type: "color"; label: string; value?: AnnotationColor; onChange: (value: AnnotationColor) => void }
  | {
      type: "select";
      label: string;
      value: string;
      options: { value: string; label: string }[];
      onChange: (value: string) => void;
    }
  | { type: "checkbox"; label: string; checked: boolean; onChange: (checked: boolean) => void };

function defaultLinearGradient(fill?: AnnotationColor): LinearGradient {
  const base = fill ?? { r: 128, g: 128, b: 128 };
  return {
    x1: 0,
    y1: 0,
    x2: 1,
    y2: 0,
    stops: [
      { offset: 0, color: base },
      { offset: 1, color: { r: 255, g: 255, b: 255 } },
    ],
  };
}

function defaultRadialGradient(fill?: AnnotationColor): RadialGradient {
  const base = fill ?? { r: 128, g: 128, b: 128 };
  return {
    cx: 0.5,
    cy: 0.5,
    r: 0.5,
    stops: [
      { offset: 0, color: base },
      { offset: 1, color: { r: 255, g: 255, b: 255 } },
    ],
  };
}

function linePatternOptions(): { value: string; label: string }[] {
  return [
    { value: "", label: t("patternSolid") },
    { value: "dashed", label: t("patternDashed") },
    { value: "dotted", label: t("patternDotted") },
    { value: "dotdashed", label: t("patternDotDashed") },
  ];
}

function fillKindOf(graphic: GraphicRectangle | GraphicEllipse | GraphicPolygon): string {
  if (graphic.fillGradient?.type === "linearGradient") return "linear";
  if (graphic.fillGradient?.type === "radialGradient") return "radial";
  const fp = (graphic.fillPattern ?? "").toLowerCase();
  if (fp.includes("none") || fp === "") return "none";
  return "solid";
}

function fillKindOptions(): { value: string; label: string }[] {
  return [
    { value: "none", label: t("fillKindNone") },
    { value: "solid", label: t("fillKindSolid") },
    { value: "linear", label: t("fillKindLinearGradient") },
    { value: "radial", label: t("fillKindRadialGradient") },
  ];
}

function fillPatternOptions(): { value: string; label: string }[] {
  return [
    { value: "none", label: t("fillKindNone") },
    { value: "solid", label: t("patternSolid") },
    { value: "horizontal", label: t("fillPatternHorizontal") },
    { value: "vertical", label: t("fillPatternVertical") },
    { value: "cross", label: t("fillPatternCross") },
  ];
}

function arrowPlacementFromSpec(spec?: string[]): "none" | "end" | "start" | "both" {
  if (!spec?.length) return "none";
  const lower = spec.map((a) => a.toLowerCase());
  const hasS = lower.some((a) => a === "start" || a === "first");
  const hasE = lower.some((a) => a === "end" || a === "last");
  if (hasS && hasE) return "both";
  if (hasS) return "start";
  if (hasE) return "end";
  return "none";
}

function arrowStyleFromSpec(spec?: string[]): "filled" | "open" | "arrow" {
  const hit = spec?.find((a) => ["filled", "open", "arrow"].includes(a.toLowerCase()));
  const t = hit?.toLowerCase();
  if (t === "open" || t === "arrow") return t;
  return "filled";
}

function buildArrowSpec(
  placement: "none" | "end" | "start" | "both",
  style: "filled" | "open" | "arrow",
): string[] | undefined {
  if (placement === "none") return undefined;
  const out: string[] = [];
  if (placement === "start" || placement === "both") out.push("start");
  if (placement === "end" || placement === "both") out.push("end");
  if (style !== "filled") out.push(style);
  return out;
}

function normalizeStrokePatternSelect(pattern?: string): string {
  if (!pattern) return "";
  const lower = pattern.toLowerCase();
  if (lower.includes("dotdashed") || lower.includes("dashdot")) return "dotdashed";
  if (lower.includes("dotted")) return "dotted";
  if (lower.includes("dashed")) return "dashed";
  return "";
}

function fillPatternSelectValue(pattern?: string): string {
  if (!pattern) return "solid";
  const lower = pattern.toLowerCase();
  if (lower.includes("none")) return "none";
  if (lower.includes("horizontal")) return "horizontal";
  if (lower.includes("vertical")) return "vertical";
  if (lower.includes("cross") && !lower.includes("diag")) return "cross";
  return "solid";
}

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

import { createDefaultGraphic } from "./modelicaProperty/createDefaultGraphic";

export { createDefaultGraphic };

interface ModelicaPropertyPanelProps {
  projectDir: string | null;
  mode: "icon" | "diagram";
  presentation?: "sidebar" | "floating";
  selectedComponent: SelectedComponent | null;
  graphics: GraphicItem[];
  selectedGraphicPath: number[] | null;
  onSelectGraphic: (path: number[] | null, additive?: boolean) => void;
  onUpdateGraphic: (path: number[], next: GraphicItem) => void;
  onAddGraphic: (graphic: GraphicItem) => void;
  onDeleteGraphic: (path: number[]) => void;
  onUpdateParam: (name: string, value: string) => void;
  onUpdatePlacement: (patch: { x?: number; y?: number; rotation?: number }) => void;
  onUpdateDeclaredType?: (typeName: string) => void;
  onUpdateComponentFlags?: (patch: { condition?: string | null; visible?: boolean | null }) => void;
  source?: string;
  modelName?: string;
  onOpenDependencyGraphSettings?: () => void;
  dependencyGraphBehavior?: DependencyGraphBehavior;
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
      {field.type === "select" && (
        <select
          value={field.value}
          onChange={(event) => field.onChange(event.target.value)}
          className="w-full rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1"
        >
          {field.options.map((opt) => (
            <option key={opt.value || "__empty"} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
      )}
      {field.type === "checkbox" && (
        <div className="flex items-center gap-2">
          <input
            type="checkbox"
            checked={field.checked}
            onChange={(event) => field.onChange(event.target.checked)}
            className="rounded border-[var(--border)]"
          />
        </div>
      )}
    </label>
  );
}

type ShapeWithGradientFill = GraphicRectangle | GraphicEllipse | GraphicPolygon;

function normalizeGradientStopOffsets(stops: GradientStop[]): GradientStop[] {
  const n = stops.length;
  if (n <= 1) {
    return stops.map((s) => ({ ...s, offset: 0 }));
  }
  return stops.map((s, i) => ({ ...s, offset: i / (n - 1) }));
}

function FillGradientEditor({
  graphic,
  onCommit,
}: {
  graphic: ShapeWithGradientFill;
  onCommit: (next: GraphicItem) => void;
}) {
  const [draggingStopIndex, setDraggingStopIndex] = useState<number | null>(null);
  const fg = graphic.fillGradient;
  if (!fg) return null;

  const commit = (mutate: (draft: ShapeWithGradientFill) => void) => {
    const next = cloneGraphic(graphic) as ShapeWithGradientFill;
    mutate(next);
    onCommit(next);
  };

  const setLinearField = (key: keyof LinearGradient, value: number) => {
    if (fg.type !== "linearGradient") return;
    commit((draft) => {
      if (draft.fillGradient?.type === "linearGradient") {
        const g = draft.fillGradient.gradient;
        if (key === "x1") g.x1 = value;
        else if (key === "y1") g.y1 = value;
        else if (key === "x2") g.x2 = value;
        else if (key === "y2") g.y2 = value;
      }
    });
  };

  const setRadialField = (key: keyof RadialGradient, value: number) => {
    if (fg.type !== "radialGradient") return;
    commit((draft) => {
      if (draft.fillGradient?.type === "radialGradient") {
        const g = draft.fillGradient.gradient;
        if (key === "cx") g.cx = value;
        else if (key === "cy") g.cy = value;
        else if (key === "r") g.r = value;
      }
    });
  };

  const setStops = (stops: GradientStop[]) => {
    commit((draft) => {
      const g = draft.fillGradient;
      if (g?.type === "linearGradient") g.gradient.stops = stops;
      else if (g?.type === "radialGradient") g.gradient.stops = stops;
    });
  };

  const stops =
    fg.type === "linearGradient" ? fg.gradient.stops : fg.type === "radialGradient" ? fg.gradient.stops : [];

  const updateStop = (index: number, patch: Partial<GradientStop>) => {
    const nextStops = stops.map((s, i) => (i === index ? { ...s, ...patch } : s));
    setStops(nextStops);
  };

  const removeStop = (index: number) => {
    if (stops.length <= 2) return;
    setStops(stops.filter((_, i) => i !== index));
  };

  const addStop = () => {
    if (stops.length === 0) {
      setStops([
        { offset: 0, color: { r: 64, g: 64, b: 64 } },
        { offset: 1, color: { r: 220, g: 220, b: 220 } },
      ]);
      return;
    }
    const last = stops[stops.length - 1]!;
    const prev = stops[stops.length - 2] ?? { offset: 0, color: last.color };
    const midOffset = (prev.offset + last.offset) / 2;
    setStops([
      ...stops.slice(0, -1),
      { offset: midOffset, color: { ...last.color }, opacity: last.opacity },
      last,
    ]);
  };

  const sortStopsByOffset = () => {
    setStops([...stops].sort((a, b) => a.offset - b.offset));
  };

  const moveStop = (from: number, to: number) => {
    if (from === to || from < 0 || to < 0 || from >= stops.length || to >= stops.length) return;
    const next = [...stops];
    const [item] = next.splice(from, 1);
    next.splice(to, 0, item!);
    setStops(normalizeGradientStopOffsets(next));
  };

  return (
    <div className="space-y-2 border-t border-[var(--border)] pt-2 mt-1">
      {fg.type === "linearGradient" && (
        <>
          <div className="text-[10px] font-medium text-[var(--text-muted)]">{t("graphicGradientLinearParams")}</div>
          <div className="grid grid-cols-2 gap-2">
            {(
              [
                ["x1", fg.gradient.x1],
                ["y1", fg.gradient.y1],
                ["x2", fg.gradient.x2],
                ["y2", fg.gradient.y2],
              ] as const
            ).map(([key, val]) => (
              <label key={key} className="block">
                <div className="mb-1 text-[var(--text-muted)]">{key}</div>
                <input
                  type="number"
                  step="any"
                  value={val}
                  onChange={(e) => setLinearField(key, Number(e.target.value))}
                  className="w-full rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1"
                />
              </label>
            ))}
          </div>
        </>
      )}
      {fg.type === "radialGradient" && (
        <>
          <div className="text-[10px] font-medium text-[var(--text-muted)]">{t("graphicGradientRadialParams")}</div>
          <div className="grid grid-cols-2 gap-2">
            {(
              [
                ["cx", fg.gradient.cx],
                ["cy", fg.gradient.cy],
                ["r", fg.gradient.r],
              ] as const
            ).map(([key, val]) => (
              <label key={key} className="block">
                <div className="mb-1 text-[var(--text-muted)]">{key}</div>
                <input
                  type="number"
                  step="any"
                  value={val}
                  onChange={(e) => setRadialField(key, Number(e.target.value))}
                  className="w-full rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1"
                />
              </label>
            ))}
          </div>
        </>
      )}
      <div className="flex items-center justify-between gap-1">
        <div className="text-[10px] font-medium text-[var(--text-muted)]">{t("graphicGradientStops")}</div>
        <button
          type="button"
          className="shrink-0 rounded border border-[var(--border)] bg-[var(--surface)] px-2 py-0.5 text-[10px] hover:bg-white/10"
          onClick={sortStopsByOffset}
        >
          {t("graphicGradientSortByOffset")}
        </button>
      </div>
      <div className="space-y-2">
        {stops.map((stop, index) => (
          <div
            key={`stop-${index}`}
            onDragOver={(e) => {
              e.preventDefault();
              e.dataTransfer.dropEffect = "move";
            }}
            onDrop={(e) => {
              e.preventDefault();
              const raw = e.dataTransfer.getData("text/plain");
              const from = Number.parseInt(raw, 10);
              if (!Number.isFinite(from) || from < 0 || from >= stops.length) {
                setDraggingStopIndex(null);
                return;
              }
              moveStop(from, index);
              setDraggingStopIndex(null);
            }}
            className={`grid gap-1 rounded border border-[var(--border)] bg-[var(--surface)] p-1.5 ${
              draggingStopIndex === index ? "opacity-60" : ""
            }`}
            style={{ gridTemplateColumns: "auto 1fr 1fr 1fr auto" }}
          >
            <span
              draggable
              title={t("graphicGradientDragReorder")}
              onDragStart={(e) => {
                e.dataTransfer.setData("text/plain", String(index));
                e.dataTransfer.effectAllowed = "move";
                setDraggingStopIndex(index);
              }}
              onDragEnd={() => setDraggingStopIndex(null)}
              className="cursor-grab self-center text-[var(--text-muted)] active:cursor-grabbing shrink-0 px-0.5"
            >
              <GripVertical className="h-3.5 w-3.5" />
            </span>
            <label className="block min-w-0">
              <div className="mb-0.5 text-[9px] text-[var(--text-muted)]">t</div>
              <input
                type="number"
                step="any"
                value={stop.offset}
                onChange={(e) => updateStop(index, { offset: Number(e.target.value) })}
                className="w-full rounded bg-[var(--bg-elevated)] border border-[var(--border)] px-1 py-0.5 text-[11px]"
              />
            </label>
            <label className="block min-w-0">
              <div className="mb-0.5 text-[9px] text-[var(--text-muted)]">RGB</div>
              <input
                type="color"
                value={colorToHex(stop.color)}
                onChange={(e) => updateStop(index, { color: hexToColor(e.target.value) })}
                className="h-7 w-full rounded border border-[var(--border)] px-0.5"
              />
            </label>
            <label className="block min-w-0">
              <div className="mb-0.5 text-[9px] text-[var(--text-muted)]">{t("graphicGradientStopOpacityPct")}</div>
              <input
                type="number"
                min={0}
                max={100}
                value={Math.round((stop.opacity ?? 1) * 100)}
                onChange={(e) => {
                  const pct = Number(e.target.value);
                  const o = Math.max(0, Math.min(100, pct)) / 100;
                  updateStop(index, { opacity: o >= 0.999 ? undefined : o });
                }}
                className="w-full rounded bg-[var(--bg-elevated)] border border-[var(--border)] px-1 py-0.5 text-[11px]"
              />
            </label>
            <button
              type="button"
              disabled={stops.length <= 2}
              title={t("graphicGradientRemoveStop")}
              className="self-end rounded bg-red-600/20 px-1.5 py-0.5 text-[11px] text-red-300 disabled:opacity-40"
              onClick={() => removeStop(index)}
            >
              −
            </button>
          </div>
        ))}
      </div>
      <button
        type="button"
        className="w-full rounded border border-[var(--border)] bg-[var(--surface)] px-2 py-1 text-[11px] hover:bg-white/10"
        onClick={addStop}
      >
        {t("graphicGradientAddStop")}
      </button>
      <p className="text-[9px] text-[var(--text-muted)] leading-tight">{t("graphicGradientDragReorderHint")}</p>
    </div>
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

  if ("thickness" in graphic && (graphic.type === "Line" || graphic.type === "BSpline")) {
    fields.push({
      type: "number",
      label: "Thickness",
      value: graphic.thickness ?? 1,
      onChange: (value) =>
        onChange(
          updateGraphicField(graphic, (item) => item as GraphicLine | GraphicBSpline, (next) => {
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

  if ("color" in graphic && (graphic.type === "Line" || graphic.type === "BSpline")) {
    fields.push({
      type: "color",
      label: "Color",
      value: graphic.color,
      onChange: (value) =>
        onChange(
          updateGraphicField(graphic, (item) => item as GraphicLine | GraphicBSpline, (next) => {
            next.color = value;
          }),
        ),
    });
  }

  if (graphic.type === "Line" || graphic.type === "BSpline") {
    fields.push({
      type: "select",
      label: t("graphicLinePattern"),
      value: normalizeStrokePatternSelect(graphic.pattern),
      options: linePatternOptions(),
      onChange: (value) =>
        onChange(
          updateGraphicField(graphic, (item) => item as GraphicLine | GraphicBSpline, (next) => {
            next.pattern = value || undefined;
          }),
        ),
    });
  }

  if (graphic.type === "Line" || graphic.type === "BSpline") {
    const placement = arrowPlacementFromSpec(graphic.arrow);
    fields.push({
      type: "select",
      label: t("graphicArrowHeads"),
      value: placement,
      options: [
        { value: "none", label: t("graphicArrowHeadsNone") },
        { value: "end", label: t("graphicArrowHeadsEnd") },
        { value: "start", label: t("graphicArrowHeadsStart") },
        { value: "both", label: t("graphicArrowHeadsBoth") },
      ],
      onChange: (value) =>
        onChange(
          updateGraphicField(graphic, (item) => item as GraphicLine | GraphicBSpline, (next) => {
            const p = value as "none" | "end" | "start" | "both";
            if (p === "none") {
              delete next.arrow;
              delete next.arrowSize;
              return;
            }
            const s = arrowStyleFromSpec(next.arrow);
            const spec = buildArrowSpec(p, s);
            if (spec) next.arrow = spec;
            else delete next.arrow;
          }),
        ),
    });
    if (placement !== "none") {
      fields.push({
        type: "select",
        label: t("graphicArrowStyle"),
        value: arrowStyleFromSpec(graphic.arrow),
        options: [
          { value: "filled", label: t("graphicArrowStyleFilled") },
          { value: "open", label: t("graphicArrowStyleOpen") },
          { value: "arrow", label: t("graphicArrowStyleLine") },
        ],
        onChange: (value) =>
          onChange(
            updateGraphicField(graphic, (item) => item as GraphicLine | GraphicBSpline, (next) => {
              const p = arrowPlacementFromSpec(next.arrow);
              const s = (value || "filled") as "filled" | "open" | "arrow";
              const spec = buildArrowSpec(p, s);
              if (spec) next.arrow = spec;
              else delete next.arrow;
            }),
          ),
      });
      fields.push({
        type: "number",
        label: t("graphicArrowSize"),
        value: graphic.arrowSize ?? 10,
        onChange: (value) =>
          onChange(
            updateGraphicField(graphic, (item) => item as GraphicLine | GraphicBSpline, (next) => {
              next.arrowSize = value;
            }),
          ),
      });
    }
  }

  if (graphic.type === "Rectangle") {
    fields.push({
      type: "select",
      label: t("graphicBorderPattern"),
      value: normalizeStrokePatternSelect(graphic.borderPattern),
      options: linePatternOptions(),
      onChange: (value) =>
        onChange(
          updateGraphicField(graphic, (item) => item as GraphicRectangle, (next) => {
            next.borderPattern = value || undefined;
          }),
        ),
    });
  }

  if (graphic.type === "Ellipse" || graphic.type === "Polygon") {
    fields.push({
      type: "select",
      label: t("graphicLinePattern"),
      value: normalizeStrokePatternSelect(graphic.linePattern),
      options: linePatternOptions(),
      onChange: (value) =>
        onChange(
          updateGraphicField(graphic, (item) => item as GraphicEllipse | GraphicPolygon, (next) => {
            next.linePattern = value || undefined;
          }),
        ),
    });
  }

  if (graphic.type === "Rectangle" || graphic.type === "Ellipse" || graphic.type === "Polygon") {
    const filled = graphic as GraphicRectangle | GraphicEllipse | GraphicPolygon;
    fields.push({
      type: "select",
      label: t("graphicFillKind"),
      value: fillKindOf(filled),
      options: fillKindOptions(),
      onChange: (value) =>
        onChange(
          updateGraphicField(graphic, (item) => item as GraphicRectangle | GraphicEllipse | GraphicPolygon, (next) => {
            if (value === "none") {
              next.fillGradient = undefined;
              next.fillPattern = "none";
            } else if (value === "solid") {
              next.fillGradient = undefined;
              next.fillPattern = next.fillPattern && next.fillPattern.toLowerCase().includes("none") ? "solid" : (next.fillPattern ?? "solid");
            } else if (value === "linear") {
              next.fillPattern = "solid";
              next.fillGradient = { type: "linearGradient", gradient: defaultLinearGradient(next.fillColor) };
            } else if (value === "radial") {
              next.fillPattern = "solid";
              next.fillGradient = { type: "radialGradient", gradient: defaultRadialGradient(next.fillColor) };
            }
          }),
        ),
    });
    if (fillKindOf(filled) === "solid") {
      fields.push({
        type: "select",
        label: t("graphicFillPattern"),
        value: fillPatternSelectValue(filled.fillPattern),
        options: fillPatternOptions(),
        onChange: (value) =>
          onChange(
            updateGraphicField(graphic, (item) => item as GraphicRectangle | GraphicEllipse | GraphicPolygon, (next) => {
              next.fillPattern = value;
            }),
          ),
      });
    }
  }

  if (graphic.type === "Text" && "fillPattern" in graphic) {
    fields.push({
      type: "select",
      label: t("graphicFillPattern"),
      value: fillPatternSelectValue(graphic.fillPattern),
      options: fillPatternOptions(),
      onChange: (value) =>
        onChange(
          updateGraphicField(graphic, (item) => item as GraphicText, (next) => {
            next.fillPattern = value;
          }),
        ),
    });
  }

  fields.push({
    type: "number",
    label: t("graphicOpacity"),
    value: Math.round((graphic.opacity ?? 1) * 100),
    onChange: (value) =>
      onChange(
        updateGraphicField(graphic, (item) => item as GraphicItem, (next) => {
          next.opacity = Math.max(0, Math.min(100, value)) / 100;
        }),
      ),
  });

  fields.push({
    type: "checkbox",
    label: t("graphicMirrorX"),
    checked: !!graphic.mirrorX,
    onChange: (checked) =>
      onChange(
        updateGraphicField(graphic, (item) => item as GraphicItem, (next) => {
          next.mirrorX = checked;
        }),
      ),
  });

  fields.push({
    type: "checkbox",
    label: t("graphicMirrorY"),
    checked: !!graphic.mirrorY,
    onChange: (checked) =>
      onChange(
        updateGraphicField(graphic, (item) => item as GraphicItem, (next) => {
          next.mirrorY = checked;
        }),
      ),
  });

  return fields;
}

export function ModelicaPropertyPanel({
  projectDir,
  mode,
  presentation = "sidebar",
  selectedComponent,
  graphics,
  selectedGraphicPath,
  onSelectGraphic,
  onUpdateGraphic,
  onAddGraphic,
  onDeleteGraphic,
  onUpdateParam,
  onUpdatePlacement,
  onUpdateDeclaredType,
  onUpdateComponentFlags,
  source,
  modelName,
  onOpenDependencyGraphSettings,
  dependencyGraphBehavior,
}: ModelicaPropertyPanelProps) {
  const [typeInfo, setTypeInfo] = useState<ComponentTypeInfo | null>(null);
  const [redeclareDraft, setRedeclareDraft] = useState("");
  const [conditionDraft, setConditionDraft] = useState("");
  const [visibleDraft, setVisibleDraft] = useState(true);
  const [showDepGraph, setShowDepGraph] = useState(false);
  const [depGraphModal, setDepGraphModal] = useState(false);
  const [depPaperHandle, setDepPaperHandle] = useState<JointPaperHandle | null>(null);

  const handleDepZoomIn = useCallback(() => depPaperHandle?.zoomIn(), [depPaperHandle]);
  const handleDepZoomOut = useCallback(() => depPaperHandle?.zoomOut(), [depPaperHandle]);
  const handleDepFitView = useCallback(() => depPaperHandle?.fitView(), [depPaperHandle]);

  useEffect(() => {
    setShowDepGraph(false);
    setDepPaperHandle(null);
  }, [selectedComponent?.name]);

  useEffect(() => {
    if (!selectedComponent) {
      setRedeclareDraft("");
      setConditionDraft("");
      setVisibleDraft(true);
      return;
    }
    setRedeclareDraft(selectedComponent.typeName);
    setConditionDraft(selectedComponent.condition ?? "");
    setVisibleDraft(selectedComponent.visible !== false);
  }, [selectedComponent]);

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

  const editPath =
    selectedGraphicPath != null && selectedGraphicPath.length > 0 ? selectedGraphicPath : null;
  const currentGraphic = editPath ? getGraphicAtPath(graphics, editPath) : null;
  const placement = selectedComponent?.placement?.transformation;
  const origin = placement?.origin ?? { x: 0, y: 0 };
  const rotation = placement?.rotation ?? 0;
  const componentParams = new Map((selectedComponent?.params ?? []).map((item) => [item.name, item.value]));
  const currentGraphicFields =
    currentGraphic && editPath ? graphicFields(currentGraphic, (next) => onUpdateGraphic(editPath, next)) : [];
  const showGraphicLibrary = presentation === "sidebar";
  const showComponentSection =
    Boolean(selectedComponent) &&
    (mode === "icon" || groupedParams.length > 0 || (mode === "diagram" && Boolean(selectedComponent)));
  const showGraphicsSection = mode === "icon" || currentGraphic != null || presentation === "sidebar";
  const showDiagramAnnotSection =
    mode === "diagram" &&
    Boolean(selectedComponent) &&
    Boolean(onUpdateDeclaredType || onUpdateComponentFlags);
  const hasVisibleDiagramDetails =
    showComponentSection || currentGraphic != null || showDiagramAnnotSection;
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
            {mode === "diagram" && selectedComponent.replaceable && onUpdateDeclaredType && (
              <div className="space-y-2 border border-[var(--border)] rounded p-2">
                <div className="text-[var(--text-muted)]">{t("diagramRedeclareType")}</div>
                {selectedComponent.constrainedbyType ?
                  <div className="text-[10px] text-[var(--text-muted)]">
                    {t("diagramConstrainedBy")}: {selectedComponent.constrainedbyType}
                  </div>
                : null}
                <input
                  type="text"
                  value={redeclareDraft}
                  onChange={(e) => setRedeclareDraft(e.target.value)}
                  className="w-full rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1"
                />
                <button
                  type="button"
                  className="w-full rounded border border-[var(--border)] bg-[var(--surface)] py-1 hover:bg-white/10"
                  onClick={() => onUpdateDeclaredType(redeclareDraft.trim())}
                >
                  {t("diagramApplyRedeclare")}
                </button>
              </div>
            )}
            {mode === "diagram" && onUpdateComponentFlags && (
              <div className="space-y-2 border border-[var(--border)] rounded p-2">
                <div className="text-[var(--text-muted)]">{t("diagramAnnotationFlags")}</div>
                <label className="block">
                  <div className="mb-1 text-[var(--text-muted)]">{t("diagramConditionExpr")}</div>
                  <input
                    type="text"
                    value={conditionDraft}
                    onChange={(e) => setConditionDraft(e.target.value)}
                    className="w-full rounded bg-[var(--surface)] border border-[var(--border)] px-2 py-1"
                  />
                </label>
                <label className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={visibleDraft}
                    onChange={(e) => setVisibleDraft(e.target.checked)}
                  />
                  <span>{t("diagramVisibleInDiagram")}</span>
                </label>
                <button
                  type="button"
                  className="w-full rounded border border-[var(--border)] bg-[var(--surface)] py-1 hover:bg-white/10"
                  onClick={() =>
                    onUpdateComponentFlags({
                      condition: conditionDraft.trim() || null,
                      visible: visibleDraft,
                    })
                  }
                >
                  {t("diagramApplyFlags")}
                </button>
              </div>
            )}
          </section>
        )}

        {mode === "diagram" && selectedComponent && source && modelName && (
          <section className="space-y-1">
            <div className="flex items-center justify-between">
              <button
                type="button"
                className="flex items-center gap-1 text-[var(--text)] hover:text-[var(--primary)]"
                onClick={() => setShowDepGraph((v) => !v)}
              >
                <svg
                  className={`h-3 w-3 shrink-0 transition-transform ${showDepGraph ? "rotate-90" : ""}`}
                  viewBox="0 0 12 12"
                  fill="currentColor"
                >
                  <path d="M4 2l5 4-5 4z" />
                </svg>
                <span className="font-medium">{t("viewDependencyGraph")}</span>
              </button>
              {showDepGraph && (
                <div className="flex items-center gap-0.5">
                  <button type="button" className="p-1 rounded hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]" title={t("zoomIn")} onClick={handleDepZoomIn}>
                    <ZoomIn className="h-3 w-3" />
                  </button>
                  <button type="button" className="p-1 rounded hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]" title={t("zoomOut")} onClick={handleDepZoomOut}>
                    <ZoomOut className="h-3 w-3" />
                  </button>
                  <button type="button" className="p-1 rounded hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]" title={t("fitView")} onClick={handleDepFitView}>
                    <Maximize2 className="h-3 w-3" />
                  </button>
                  <div className="w-px h-3 bg-[var(--border)] mx-0.5" />
                  <button type="button" className="p-1 rounded hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]" title={t("expandToWindow")} onClick={() => setDepGraphModal(true)}>
                    <Maximize className="h-3 w-3" />
                  </button>
                  {onOpenDependencyGraphSettings ? (
                    <>
                      <div className="w-px h-3 bg-[var(--border)] mx-0.5" />
                      <button
                        type="button"
                        className="p-1 rounded hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]"
                        title={t("dependencyGraphOpenSettings")}
                        aria-label={t("dependencyGraphOpenSettings")}
                        onClick={() => onOpenDependencyGraphSettings()}
                      >
                        <Settings className="h-3 w-3" />
                      </button>
                    </>
                  ) : null}
                </div>
              )}
            </div>
            {showDepGraph && (
              <div className="border border-[var(--border)] rounded overflow-hidden relative" style={{ height: 260 }}>
                <EquationGraphView
                  code={source}
                  modelName={modelName}
                  projectDir={projectDir}
                  layoutOptions={{ algorithm: "layered", direction: "RIGHT" }}
                  dependencyGraphBehavior={dependencyGraphBehavior}
                  onReady={setDepPaperHandle}
                />
              </div>
            )}
            {depGraphModal && (
              <DependencyGraphModal
                open={depGraphModal}
                onClose={() => setDepGraphModal(false)}
                code={source}
                modelName={modelName}
                projectDir={projectDir}
                onOpenDependencyGraphSettings={onOpenDependencyGraphSettings}
                dependencyGraphBehavior={dependencyGraphBehavior}
              />
            )}
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
                    editPath != null && editPath[0] === index
                      ? "border-primary bg-primary/10"
                      : "border-[var(--border)] bg-[var(--surface)]"
                  }`}
                  onClick={() => onSelectGraphic([index], false)}
                >
                  {graphic.type === "Group" ? t("shapeGroup") : graphic.type} #{index + 1}
                </button>
              ))}
            </div>
            {currentGraphic && editPath && (
              <div className="border border-[var(--border)] rounded p-2 space-y-2">
                <div className="flex items-center justify-between">
                  <span className="font-medium">
                    {currentGraphic.type === "Group" ? t("shapeGroup") : currentGraphic.type}
                  </span>
                  <button
                    type="button"
                    className="rounded bg-red-600/20 px-2 py-1 text-red-300"
                    onClick={() => onDeleteGraphic(editPath)}
                  >
                    {t("deleteTest")}
                  </button>
                </div>
                {currentGraphic.type === "Group" && (
                  <div className="text-[var(--text-muted)]">
                    {t("groupChildren")}: {currentGraphic.children.length}
                  </div>
                )}
                <div className="grid grid-cols-1 gap-2">
                  {currentGraphicFields.map((field, index) => renderField(field, `field-${index}`))}
                </div>
                {(currentGraphic.type === "Rectangle" ||
                  currentGraphic.type === "Ellipse" ||
                  currentGraphic.type === "Polygon") &&
                  currentGraphic.fillGradient && (
                    <FillGradientEditor
                      graphic={currentGraphic}
                      onCommit={(next) => onUpdateGraphic(editPath, next)}
                    />
                  )}
                {"extent" in currentGraphic && currentGraphic.extent && (
                  <div className="grid grid-cols-2 gap-2">
                    {(["p1", "p2"] as const).flatMap((corner) => ([
                      <label key={`${corner}.x`} className="block">
                        <div className="mb-1 text-[var(--text-muted)]">{corner}.x</div>
                        <input
                          type="number"
                          value={currentGraphic.extent?.[corner].x ?? 0}
                          onChange={(e) => {
                            const next = cloneGraphic(currentGraphic);
                            if ("extent" in next && next.extent) {
                              next.extent[corner].x = Number(e.target.value);
                            }
                            onUpdateGraphic(editPath, next);
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
                            const next = cloneGraphic(currentGraphic);
                            if ("extent" in next && next.extent) {
                              next.extent[corner].y = Number(e.target.value);
                            }
                            onUpdateGraphic(editPath, next);
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
                              onUpdateGraphic(editPath, next);
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
                              onUpdateGraphic(editPath, next);
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
