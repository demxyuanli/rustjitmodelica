import React from "react";

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

export interface GraphicBitmap {
  type: "Bitmap";
  extent?: AnnotationExtent;
  fileName?: string;
  imageSource?: string;
  rotation?: number;
  origin?: AnnotationPoint;
}

export type GraphicItem =
  | GraphicLine
  | GraphicRectangle
  | GraphicEllipse
  | GraphicPolygon
  | GraphicText
  | GraphicBitmap;

export interface CoordinateSystem {
  extent?: AnnotationExtent;
  preserveAspectRatio?: boolean;
  initialScale?: number;
}

export interface IconDiagramAnnotation {
  coordinateSystem?: CoordinateSystem;
  graphics: GraphicItem[];
}

export interface LineAnnotation {
  points: AnnotationPoint[];
  color?: AnnotationColor;
  thickness?: number;
  pattern?: string;
  smooth?: string;
}

export interface GraphicBounds {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

export interface ConnectorAnchor {
  id: string;
  point: AnnotationPoint;
}

export const DEFAULT_ICON_SIZE = 40;

export function colorToCSS(c?: AnnotationColor): string {
  if (!c) return "currentColor";
  return `rgb(${c.r},${c.g},${c.b})`;
}

function isFilled(fillPattern?: string): boolean {
  if (!fillPattern) return false;
  const lower = fillPattern.toLowerCase();
  return (
    lower.includes("solid") ||
    lower.includes("horizontal") ||
    lower.includes("vertical") ||
    lower.includes("cross")
  );
}

function defaultExtent(): AnnotationExtent {
  return { p1: { x: -100, y: -100 }, p2: { x: 100, y: 100 } };
}

export function getCoordinateExtent(cs?: CoordinateSystem): AnnotationExtent {
  return cs?.extent ?? defaultExtent();
}

function rotatePoint(point: AnnotationPoint, rotation = 0, origin?: AnnotationPoint): AnnotationPoint {
  if (!rotation) return point;
  const center = origin ?? { x: 0, y: 0 };
  const radians = (rotation * Math.PI) / 180;
  const dx = point.x - center.x;
  const dy = point.y - center.y;
  return {
    x: center.x + dx * Math.cos(radians) - dy * Math.sin(radians),
    y: center.y + dx * Math.sin(radians) + dy * Math.cos(radians),
  };
}

export function coordToSvg(
  p: AnnotationPoint,
  cs?: CoordinateSystem,
  svgW = DEFAULT_ICON_SIZE,
  svgH = DEFAULT_ICON_SIZE,
): { x: number; y: number } {
  const ext = getCoordinateExtent(cs);
  const cw = Math.abs(ext.p2.x - ext.p1.x) || 200;
  const ch = Math.abs(ext.p2.y - ext.p1.y) || 200;
  const minX = Math.min(ext.p1.x, ext.p2.x);
  const maxY = Math.max(ext.p1.y, ext.p2.y);
  return {
    x: ((p.x - minX) / cw) * svgW,
    y: ((maxY - p.y) / ch) * svgH,
  };
}

export function svgToCoord(
  p: AnnotationPoint,
  cs?: CoordinateSystem,
  svgW = DEFAULT_ICON_SIZE,
  svgH = DEFAULT_ICON_SIZE,
): AnnotationPoint {
  const ext = getCoordinateExtent(cs);
  const cw = Math.abs(ext.p2.x - ext.p1.x) || 200;
  const ch = Math.abs(ext.p2.y - ext.p1.y) || 200;
  const minX = Math.min(ext.p1.x, ext.p2.x);
  const maxY = Math.max(ext.p1.y, ext.p2.y);
  return {
    x: minX + (p.x / svgW) * cw,
    y: maxY - (p.y / svgH) * ch,
  };
}

export function extentToSvgRect(
  ext: AnnotationExtent,
  cs?: CoordinateSystem,
  svgW = DEFAULT_ICON_SIZE,
  svgH = DEFAULT_ICON_SIZE,
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

function transformPoints(points: AnnotationPoint[], rotation?: number, origin?: AnnotationPoint): AnnotationPoint[] {
  return points.map((point) => rotatePoint(point, rotation, origin));
}

export function getGraphicBounds(item: GraphicItem): GraphicBounds | null {
  let points: AnnotationPoint[] = [];
  switch (item.type) {
    case "Line":
    case "Polygon":
      points = transformPoints(item.points, item.rotation, item.origin);
      break;
    case "Rectangle":
    case "Ellipse":
    case "Text":
    case "Bitmap":
      if (!item.extent) return null;
      points = transformPoints(
        [
          item.extent.p1,
          item.extent.p2,
          { x: item.extent.p1.x, y: item.extent.p2.y },
          { x: item.extent.p2.x, y: item.extent.p1.y },
        ],
        item.rotation,
        item.origin,
      );
      break;
    default:
      return null;
  }
  if (points.length === 0) return null;
  return {
    minX: Math.min(...points.map((point) => point.x)),
    minY: Math.min(...points.map((point) => point.y)),
    maxX: Math.max(...points.map((point) => point.x)),
    maxY: Math.max(...points.map((point) => point.y)),
  };
}

export function getConnectorAnchors(item: GraphicItem): ConnectorAnchor[] {
  const bounds = getGraphicBounds(item);
  if (!bounds) return [];
  const cx = (bounds.minX + bounds.maxX) / 2;
  const cy = (bounds.minY + bounds.maxY) / 2;
  return [
    { id: "top", point: { x: cx, y: bounds.maxY } },
    { id: "right", point: { x: bounds.maxX, y: cy } },
    { id: "bottom", point: { x: cx, y: bounds.minY } },
    { id: "left", point: { x: bounds.minX, y: cy } },
  ];
}

export function translateGraphicItem(item: GraphicItem, delta: AnnotationPoint): GraphicItem {
  const movePoint = (point: AnnotationPoint) => ({ x: point.x + delta.x, y: point.y + delta.y });
  switch (item.type) {
    case "Line":
      return {
        ...item,
        points: item.points.map(movePoint),
        origin: item.origin ? movePoint(item.origin) : item.origin,
      };
    case "Polygon":
      return {
        ...item,
        points: item.points.map(movePoint),
        origin: item.origin ? movePoint(item.origin) : item.origin,
      };
    case "Rectangle":
    case "Ellipse":
    case "Text":
    case "Bitmap":
      return {
        ...item,
        extent: item.extent
          ? {
              p1: movePoint(item.extent.p1),
              p2: movePoint(item.extent.p2),
            }
          : item.extent,
        origin: item.origin ? movePoint(item.origin) : item.origin,
      };
    default:
      return item;
  }
}

function pointDistance(a: AnnotationPoint, b: AnnotationPoint): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

function distanceToSegment(point: AnnotationPoint, a: AnnotationPoint, b: AnnotationPoint): number {
  const lengthSquared = (b.x - a.x) ** 2 + (b.y - a.y) ** 2;
  if (lengthSquared === 0) return pointDistance(point, a);
  const t = Math.max(
    0,
    Math.min(1, ((point.x - a.x) * (b.x - a.x) + (point.y - a.y) * (b.y - a.y)) / lengthSquared),
  );
  return pointDistance(point, {
    x: a.x + t * (b.x - a.x),
    y: a.y + t * (b.y - a.y),
  });
}

export function findGraphicAtPoint(
  graphics: GraphicItem[],
  point: AnnotationPoint,
  tolerance = 8,
): number {
  for (let index = graphics.length - 1; index >= 0; index -= 1) {
    const item = graphics[index];
    if (item.type === "Line" || item.type === "Polygon") {
      const points = transformPoints(item.points, item.rotation, item.origin);
      if (points.length < 2) continue;
      const hit = points.some((current, currentIndex) => {
        if (currentIndex === 0) return false;
        return distanceToSegment(point, points[currentIndex - 1], current) <= tolerance;
      });
      if (hit) return index;
    } else {
      const bounds = getGraphicBounds(item);
      if (
        bounds &&
        point.x >= bounds.minX - tolerance &&
        point.x <= bounds.maxX + tolerance &&
        point.y >= bounds.minY - tolerance &&
        point.y <= bounds.maxY + tolerance
      ) {
        return index;
      }
    }
  }
  return -1;
}

function replaceTemplateTokens(text: string, instanceName?: string): string {
  let value = text;
  if (instanceName) {
    value = value.replace(/%name/gi, instanceName);
  }
  return value.replace(/%[a-zA-Z.]+/g, "");
}

function renderGraphicItem(
  item: GraphicItem,
  idx: number,
  cs?: CoordinateSystem,
  svgW = DEFAULT_ICON_SIZE,
  svgH = DEFAULT_ICON_SIZE,
  instanceName?: string,
): React.ReactNode {
  const bounds = getGraphicBounds(item);
  const rotation = item.rotation ?? 0;
  const origin = item.origin;
  const transform =
    bounds && rotation
      ? (() => {
          const center = coordToSvg(origin ?? { x: (bounds.minX + bounds.maxX) / 2, y: (bounds.minY + bounds.maxY) / 2 }, cs, svgW, svgH);
          return `rotate(${-rotation} ${center.x} ${center.y})`;
        })()
      : undefined;

  switch (item.type) {
    case "Line": {
      if (item.points.length < 2) return null;
      const pts = item.points.map((point) => coordToSvg(point, cs, svgW, svgH));
      const d = pts.map((point, pointIndex) => `${pointIndex === 0 ? "M" : "L"} ${point.x} ${point.y}`).join(" ");
      return (
        <g key={idx} transform={transform}>
          <path d={d} stroke={colorToCSS(item.color)} strokeWidth={item.thickness ?? 1} fill="none" />
        </g>
      );
    }
    case "Rectangle": {
      if (!item.extent) return null;
      const rect = extentToSvgRect(item.extent, cs, svgW, svgH);
      return (
        <g key={idx} transform={transform}>
          <rect
            x={rect.x}
            y={rect.y}
            width={rect.width}
            height={rect.height}
            rx={item.radius ?? 0}
            stroke={colorToCSS(item.lineColor)}
            strokeWidth={item.lineThickness ?? 1}
            fill={isFilled(item.fillPattern) ? colorToCSS(item.fillColor) : "none"}
          />
        </g>
      );
    }
    case "Ellipse": {
      if (!item.extent) return null;
      const rect = extentToSvgRect(item.extent, cs, svgW, svgH);
      return (
        <g key={idx} transform={transform}>
          <ellipse
            cx={rect.x + rect.width / 2}
            cy={rect.y + rect.height / 2}
            rx={rect.width / 2}
            ry={rect.height / 2}
            stroke={colorToCSS(item.lineColor)}
            strokeWidth={item.lineThickness ?? 1}
            fill={isFilled(item.fillPattern) ? colorToCSS(item.fillColor) : "none"}
          />
        </g>
      );
    }
    case "Polygon": {
      if (item.points.length < 2) return null;
      const pts = item.points.map((point) => coordToSvg(point, cs, svgW, svgH));
      return (
        <g key={idx} transform={transform}>
          <polygon
            points={pts.map((point) => `${point.x},${point.y}`).join(" ")}
            stroke={colorToCSS(item.lineColor)}
            strokeWidth={item.lineThickness ?? 1}
            fill={isFilled(item.fillPattern) ? colorToCSS(item.fillColor) : "none"}
          />
        </g>
      );
    }
    case "Text": {
      if (!item.extent) return null;
      const rect = extentToSvgRect(item.extent, cs, svgW, svgH);
      const text = replaceTemplateTokens(item.textString ?? "", instanceName);
      if (!text.trim()) return null;
      const fontSize = Math.min(item.fontSize ?? 10, rect.height * 0.8, 18);
      let anchor: "start" | "middle" | "end" = "middle";
      if (item.horizontalAlignment === "TextAlignment.Left") anchor = "start";
      if (item.horizontalAlignment === "TextAlignment.Right") anchor = "end";
      const x =
        anchor === "start"
          ? rect.x + 2
          : anchor === "end"
            ? rect.x + rect.width - 2
            : rect.x + rect.width / 2;
      return (
        <g key={idx} transform={transform}>
          <text
            x={x}
            y={rect.y + rect.height / 2}
            textAnchor={anchor}
            dominantBaseline="central"
            fontSize={fontSize}
            fill={colorToCSS(item.textColor ?? item.lineColor)}
            fontFamily={item.fontName || "sans-serif"}
          >
            {text}
          </text>
        </g>
      );
    }
    case "Bitmap": {
      if (!item.extent) return null;
      const rect = extentToSvgRect(item.extent, cs, svgW, svgH);
      const href = item.fileName || item.imageSource;
      if (!href) return null;
      return (
        <g key={idx} transform={transform}>
          <image href={href} x={rect.x} y={rect.y} width={rect.width} height={rect.height} preserveAspectRatio="none" />
        </g>
      );
    }
    default:
      return null;
  }
}

function renderSelection(
  item: GraphicItem,
  cs?: CoordinateSystem,
  svgW = DEFAULT_ICON_SIZE,
  svgH = DEFAULT_ICON_SIZE,
): React.ReactNode {
  const bounds = getGraphicBounds(item);
  if (!bounds) return null;
  const p1 = coordToSvg({ x: bounds.minX, y: bounds.minY }, cs, svgW, svgH);
  const p2 = coordToSvg({ x: bounds.maxX, y: bounds.maxY }, cs, svgW, svgH);
  const rect = {
    x: Math.min(p1.x, p2.x),
    y: Math.min(p1.y, p2.y),
    width: Math.abs(p2.x - p1.x),
    height: Math.abs(p2.y - p1.y),
  };
  const anchors = getConnectorAnchors(item);
  return (
    <>
      <rect
        x={rect.x}
        y={rect.y}
        width={rect.width}
        height={rect.height}
        fill="none"
        stroke="#0ea5e9"
        strokeWidth={1}
        strokeDasharray="4 3"
      />
      {anchors.map((anchor) => {
        const point = coordToSvg(anchor.point, cs, svgW, svgH);
        return <circle key={anchor.id} cx={point.x} cy={point.y} r={4} fill="#0ea5e9" stroke="#fff" strokeWidth={1} />;
      })}
    </>
  );
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
        {icon.graphics.map((graphic, index) =>
          renderGraphicItem(graphic, index, icon.coordinateSystem, size, size, instanceName),
        )}
      </g>
    </svg>
  );
}

export function AnnotationGraphicsSvg({
  annotation,
  size,
  instanceName,
  selectedGraphicIndex = -1,
  className,
}: {
  annotation: IconDiagramAnnotation;
  size: { width: number; height: number };
  instanceName?: string;
  selectedGraphicIndex?: number;
  className?: string;
}) {
  const width = Math.max(1, size.width);
  const height = Math.max(1, size.height);
  const selectedGraphic =
    selectedGraphicIndex >= 0 ? annotation.graphics[selectedGraphicIndex] : null;
  return (
    <svg
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      className={className ?? "block pointer-events-none"}
    >
      {annotation.graphics.map((graphic, index) =>
        renderGraphicItem(graphic, index, annotation.coordinateSystem, width, height, instanceName),
      )}
      {selectedGraphic && renderSelection(selectedGraphic, annotation.coordinateSystem, width, height)}
    </svg>
  );
}

export { CONNECTOR_COLORS } from "./diagramConnectorColors";
import { CONNECTOR_COLORS } from "./diagramConnectorColors";

export function connectorHandleStyle(
  kind?: string,
  side: "left" | "right" = "left",
): React.CSSProperties {
  const color = (kind && CONNECTOR_COLORS[kind]) || "var(--text-muted)";
  const size = 6;
  const offset = -(size / 2);
  const base: React.CSSProperties = {
    width: size,
    height: size,
    borderRadius: 0,
    backgroundColor: color,
    border: "none",
  };
  return side === "left" ? { ...base, left: offset } : { ...base, right: offset };
}
