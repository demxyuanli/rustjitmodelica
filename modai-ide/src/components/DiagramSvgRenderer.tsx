import React from "react";

// ---------------------------------------------------------------------------
// Types for Modelica graphical annotations
// ---------------------------------------------------------------------------

export interface AnnotationPoint { x: number; y: number }
export interface AnnotationExtent { p1: AnnotationPoint; p2: AnnotationPoint }
export interface AnnotationColor { r: number; g: number; b: number }

export interface GraphicLine {
  type: "Line";
  points: AnnotationPoint[];
  color?: AnnotationColor;
  thickness?: number;
  pattern?: string;
  smooth?: string;
  arrow?: string[];
  arrowSize?: number;
  rotation?: number;
  origin?: AnnotationPoint;
}

export interface GraphicRectangle {
  type: "Rectangle";
  extent?: AnnotationExtent;
  lineColor?: AnnotationColor;
  fillColor?: AnnotationColor;
  fillPattern?: string;
  borderPattern?: string;
  lineThickness?: number;
  radius?: number;
  rotation?: number;
  origin?: AnnotationPoint;
}

export interface GraphicEllipse {
  type: "Ellipse";
  extent?: AnnotationExtent;
  lineColor?: AnnotationColor;
  fillColor?: AnnotationColor;
  fillPattern?: string;
  startAngle?: number;
  endAngle?: number;
  lineThickness?: number;
  rotation?: number;
  origin?: AnnotationPoint;
}

export interface GraphicPolygon {
  type: "Polygon";
  points: AnnotationPoint[];
  lineColor?: AnnotationColor;
  fillColor?: AnnotationColor;
  fillPattern?: string;
  lineThickness?: number;
  smooth?: string;
  rotation?: number;
  origin?: AnnotationPoint;
}

export interface GraphicText {
  type: "Text";
  extent?: AnnotationExtent;
  textString?: string;
  fontSize?: number;
  fontName?: string;
  textColor?: AnnotationColor;
  lineColor?: AnnotationColor;
  fillColor?: AnnotationColor;
  horizontalAlignment?: string;
  fillPattern?: string;
  rotation?: number;
  origin?: AnnotationPoint;
}

export type GraphicItem = GraphicLine | GraphicRectangle | GraphicEllipse | GraphicPolygon | GraphicText;

export interface CoordinateSystem {
  extent?: AnnotationExtent;
  preserveAspectRatio?: boolean;
  initialScale?: number;
}

export interface IconDiagramAnnotation {
  coordinateSystem?: CoordinateSystem;
  graphics: GraphicItem[];
}

// ---------------------------------------------------------------------------
// SVG rendering helpers
// ---------------------------------------------------------------------------

export function colorToCSS(c?: AnnotationColor): string {
  if (!c) return "currentColor";
  return `rgb(${c.r},${c.g},${c.b})`;
}

function isFilled(fillPattern?: string): boolean {
  if (!fillPattern) return false;
  const lower = fillPattern.toLowerCase();
  return lower.includes("solid") || lower.includes("horizontal") || lower.includes("vertical") || lower.includes("cross");
}

export const DEFAULT_ICON_SIZE = 40;

function coordToSvg(
  p: AnnotationPoint,
  cs?: CoordinateSystem,
  svgW = DEFAULT_ICON_SIZE,
  svgH = DEFAULT_ICON_SIZE
): { x: number; y: number } {
  const ext = cs?.extent ?? { p1: { x: -100, y: -100 }, p2: { x: 100, y: 100 } };
  const cw = Math.abs(ext.p2.x - ext.p1.x) || 200;
  const ch = Math.abs(ext.p2.y - ext.p1.y) || 200;
  const minX = Math.min(ext.p1.x, ext.p2.x);
  const maxY = Math.max(ext.p1.y, ext.p2.y);
  return {
    x: ((p.x - minX) / cw) * svgW,
    y: ((maxY - p.y) / ch) * svgH,
  };
}

function extentToSvgRect(
  ext: AnnotationExtent,
  cs?: CoordinateSystem,
  svgW = DEFAULT_ICON_SIZE,
  svgH = DEFAULT_ICON_SIZE
) {
  const p1 = coordToSvg(ext.p1, cs, svgW, svgH);
  const p2 = coordToSvg(ext.p2, cs, svgW, svgH);
  return {
    x: Math.min(p1.x, p2.x),
    y: Math.min(p1.y, p2.y),
    width: Math.abs(p2.x - p1.x),
    height: Math.abs(p2.y - p1.y),
  };
}

function renderGraphicItem(
  item: GraphicItem,
  idx: number,
  cs?: CoordinateSystem,
  svgW = DEFAULT_ICON_SIZE,
  svgH = DEFAULT_ICON_SIZE,
  instanceName?: string
): React.ReactNode {
  switch (item.type) {
    case "Line": {
      if (item.points.length < 2) return null;
      const pts = item.points.map((p) => coordToSvg(p, cs, svgW, svgH));
      const d = pts.map((p, i) => `${i === 0 ? "M" : "L"} ${p.x} ${p.y}`).join(" ");
      return (
        <path
          key={idx}
          d={d}
          stroke={colorToCSS(item.color)}
          strokeWidth={item.thickness ?? 1}
          fill="none"
        />
      );
    }
    case "Rectangle": {
      if (!item.extent) return null;
      const r = extentToSvgRect(item.extent, cs, svgW, svgH);
      const filled = isFilled(item.fillPattern);
      return (
        <rect
          key={idx}
          x={r.x}
          y={r.y}
          width={r.width}
          height={r.height}
          rx={item.radius ?? 0}
          stroke={colorToCSS(item.lineColor)}
          strokeWidth={item.lineThickness ?? 1}
          fill={filled ? colorToCSS(item.fillColor) : "none"}
        />
      );
    }
    case "Ellipse": {
      if (!item.extent) return null;
      const r = extentToSvgRect(item.extent, cs, svgW, svgH);
      const cx = r.x + r.width / 2;
      const cy = r.y + r.height / 2;
      return (
        <ellipse
          key={idx}
          cx={cx}
          cy={cy}
          rx={r.width / 2}
          ry={r.height / 2}
          stroke={colorToCSS(item.lineColor)}
          strokeWidth={item.lineThickness ?? 1}
          fill={isFilled(item.fillPattern) ? colorToCSS(item.fillColor) : "none"}
        />
      );
    }
    case "Polygon": {
      if (item.points.length < 2) return null;
      const pts = item.points.map((p) => coordToSvg(p, cs, svgW, svgH));
      const polyPoints = pts.map((p) => `${p.x},${p.y}`).join(" ");
      return (
        <polygon
          key={idx}
          points={polyPoints}
          stroke={colorToCSS(item.lineColor)}
          strokeWidth={item.lineThickness ?? 1}
          fill={isFilled(item.fillPattern) ? colorToCSS(item.fillColor) : "none"}
        />
      );
    }
    case "Text": {
      if (!item.extent) return null;
      const r = extentToSvgRect(item.extent, cs, svgW, svgH);
      let text = item.textString ?? "";
      if (instanceName) {
        text = text.replace(/%name/gi, instanceName);
      }
      text = text.replace(/%[a-zA-Z.]+/g, "");
      if (!text.trim()) return null;
      const fontSize = Math.min(item.fontSize ?? 10, r.height * 0.8, 10);
      let anchor: "start" | "middle" | "end" = "middle";
      if (item.horizontalAlignment === "TextAlignment.Left") anchor = "start";
      if (item.horizontalAlignment === "TextAlignment.Right") anchor = "end";
      const tx = anchor === "start" ? r.x + 1 : anchor === "end" ? r.x + r.width - 1 : r.x + r.width / 2;
      return (
        <text
          key={idx}
          x={tx}
          y={r.y + r.height / 2}
          textAnchor={anchor}
          dominantBaseline="central"
          fontSize={fontSize}
          fill={colorToCSS(item.textColor ?? item.lineColor)}
          fontFamily={item.fontName || "sans-serif"}
        >
          {text}
        </text>
      );
    }
    default:
      return null;
  }
}

export function IconSvg({
  icon,
  instanceName,
  rotation,
  size = DEFAULT_ICON_SIZE,
}: {
  icon: IconDiagramAnnotation;
  instanceName?: string;
  rotation?: number;
  size?: number;
}) {
  const transform = rotation ? `rotate(${rotation} ${size / 2} ${size / 2})` : undefined;
  return (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} className="block">
      <g transform={transform}>
        {icon.graphics.map((g, i) =>
          renderGraphicItem(g, i, icon.coordinateSystem, size, size, instanceName)
        )}
      </g>
    </svg>
  );
}

// ---------------------------------------------------------------------------
// Connector handle styles
// ---------------------------------------------------------------------------

export const CONNECTOR_COLORS: Record<string, string> = {
  mechanical: "#555",
  electrical: "#2563eb",
  thermal: "#dc2626",
  fluid: "#0891b2",
  signal_input: "#16a34a",
  signal_output: "#ca8a04",
};

export interface LineAnnotation {
  points: AnnotationPoint[];
  color?: AnnotationColor;
  thickness?: number;
  pattern?: string;
  smooth?: string;
}

export function connectorHandleStyle(kind?: string, side: "left" | "right" = "left"): React.CSSProperties {
  const color = (kind && CONNECTOR_COLORS[kind]) || "var(--text-muted)";
  const base: React.CSSProperties = {
    width: 8,
    height: 8,
    borderRadius: kind === "mechanical" ? 2 : "50%",
    backgroundColor: color,
    border: "1px solid var(--border)",
  };
  if (side === "left") {
    return { ...base, left: -5 };
  }
  return { ...base, right: -5 };
}
