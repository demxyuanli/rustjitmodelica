import React from "react";
import { createBSplinePath, renderArrowheads } from "./DiagramSvgArrows";

export interface AnnotationPoint { x: number; y: number }
export interface AnnotationExtent { p1: AnnotationPoint; p2: AnnotationPoint }
export interface AnnotationColor { r: number; g: number; b: number }

/** Arrow type for line endpoints */
export type ArrowType = "none" | "arrow" | "filled" | "open" | "tshape" | "circle";

/** Fill pattern for shapes */
export type FillPattern = "solid" | "horizontal" | "vertical" | "cross" | "diagCross" | "forward" | "backward" | "none";

/** Border pattern for rectangles */
export type BorderPattern = "solid" | "dashed" | "dotted" | "dotDashed";

/** Line pattern for strokes */
export type LinePattern = "solid" | "dashed" | "dotted" | "dotDashed";

/** Gradient stop for gradient fills */
export interface GradientStop {
  offset: number;
  color: AnnotationColor;
  opacity?: number;
}

/** Linear gradient specification */
export interface LinearGradient {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  stops: GradientStop[];
}

/** Radial gradient specification */
export interface RadialGradient {
  cx: number;
  cy: number;
  r: number;
  stops: GradientStop[];
}

/** Fill definition - solid color or gradient */
export type FillDefinition =
  | { type: "solid"; color: AnnotationColor }
  | { type: "linearGradient"; gradient: LinearGradient }
  | { type: "radialGradient"; gradient: RadialGradient };

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
  fillGradient?: { type: "linearGradient"; gradient: LinearGradient } | { type: "radialGradient"; gradient: RadialGradient };
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
  fillGradient?: { type: "linearGradient"; gradient: LinearGradient } | { type: "radialGradient"; gradient: RadialGradient };
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
  fillGradient?: { type: "linearGradient"; gradient: LinearGradient } | { type: "radialGradient"; gradient: RadialGradient };
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

/** Bezier spline curve for smooth connections */
export interface GraphicBSpline {
  type: "BSpline";
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

export type GraphicItem =
  | GraphicLine
  | GraphicRectangle
  | GraphicEllipse
  | GraphicPolygon
  | GraphicText
  | GraphicBitmap
  | GraphicBSpline;

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
  // "none" 图案不填充
  if (lower === "none") return false;
  return (
    lower.includes("solid") ||
    lower.includes("horizontal") ||
    lower.includes("vertical") ||
    lower.includes("cross") ||
    lower.includes("diagcross") ||
    lower.includes("forward") ||
    lower.includes("backward")
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
    case "BSpline":
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
    case "BSpline":
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
    if (item.type === "Line" || item.type === "Polygon" || item.type === "BSpline") {
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
      
      // 获取虚线样式
      let strokeDasharray: string | undefined;
      if (item.pattern) {
        switch (item.pattern.toLowerCase()) {
          case "dashed": strokeDasharray = "8,4"; break;
          case "dotted": strokeDasharray = "2,4"; break;
          case "dotdashed": strokeDasharray = "2,4,8,4"; break;
        }
      }
      
      return (
        <g key={idx} transform={transform}>
          <path 
            d={d} 
            stroke={colorToCSS(item.color)} 
            strokeWidth={item.thickness ?? 1} 
            fill="none"
            strokeDasharray={strokeDasharray}
          />
          {item.arrow && renderArrowheads(pts, item.arrow, item.color, item.arrowSize)}
        </g>
      );
    }
    case "BSpline": {
      if (item.points.length < 2) return null;
      const pts = item.points.map((point) => coordToSvg(point, cs, svgW, svgH));
      const smooth = item.smooth === "BSpline" || item.smooth === "true";
      const d = smooth ? createBSplinePath(pts) : pts.map((point, pointIndex) => `${pointIndex === 0 ? "M" : "L"} ${point.x} ${point.y}`).join(" ");
      
      // 获取虚线样式
      let strokeDasharray: string | undefined;
      if (item.pattern) {
        switch (item.pattern.toLowerCase()) {
          case "dashed": strokeDasharray = "8,4"; break;
          case "dotted": strokeDasharray = "2,4"; break;
          case "dotdashed": strokeDasharray = "2,4,8,4"; break;
        }
      }
      
      return (
        <g key={idx} transform={transform}>
          <path 
            d={d} 
            stroke={colorToCSS(item.color)} 
            strokeWidth={item.thickness ?? 1} 
            fill="none"
            strokeDasharray={strokeDasharray}
          />
          {item.arrow && renderArrowheads(pts, item.arrow, item.color, item.arrowSize)}
        </g>
      );
    }
    case "Rectangle": {
      if (!item.extent) return null;
      const rect = extentToSvgRect(item.extent, cs, svgW, svgH);
      
      // Determine fill - gradient takes precedence over solid color
      let fill: string;
      if (item.fillGradient) {
        fill = `url(#gradient-${idx})`;
      } else if (isFilled(item.fillPattern)) {
        fill = colorToCSS(item.fillColor);
      } else {
        fill = "none";
      }
      
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
            fill={fill}
          />
        </g>
      );
    }
    case "Ellipse": {
      if (!item.extent) return null;
      const rect = extentToSvgRect(item.extent, cs, svgW, svgH);
      const cx = rect.x + rect.width / 2;
      const cy = rect.y + rect.height / 2;
      const rx = rect.width / 2;
      const ry = rect.height / 2;

      // Determine fill - gradient takes precedence over solid color
      let fill: string;
      if (item.fillGradient) {
        fill = `url(#gradient-${idx})`;
      } else if (isFilled(item.fillPattern)) {
        fill = colorToCSS(item.fillColor);
      } else {
        fill = "none";
      }

      // 检查是否为扇形/圆弧
      if (item.startAngle !== undefined && item.endAngle !== undefined) {
        const startAngle = item.startAngle;
        const endAngle = item.endAngle;
        const startRad = (startAngle * Math.PI) / 180;
        const endRad = (endAngle * Math.PI) / 180;

        // SVG 坐标系 Y 轴向下，需要翻转角度
        const svgStartRad = -startRad;
        const svgEndRad = -endRad;

        const x1 = cx + rx * Math.cos(svgStartRad);
        const y1 = cy + ry * Math.sin(svgStartRad);
        const x2 = cx + rx * Math.cos(svgEndRad);
        const y2 = cy + ry * Math.sin(svgEndRad);

        const largeArc = Math.abs(endRad - startRad) > Math.PI ? 1 : 0;
        const sweep = (endRad > startAngle) ? 0 : 1; // SVG Y 轴翻转

        // 扇形路径：从中心到起点，沿弧到终点，回到中心
        const d = `M ${cx} ${cy} L ${x1} ${y1} A ${rx} ${ry} 0 ${largeArc} ${sweep} ${x2} ${y2} Z`;

        return (
          <g key={idx} transform={transform}>
            <path
              d={d}
              stroke={colorToCSS(item.lineColor)}
              strokeWidth={item.lineThickness ?? 1}
              fill={fill}
            />
          </g>
        );
      }

      // 完整椭圆
      return (
        <g key={idx} transform={transform}>
          <ellipse
            cx={cx}
            cy={cy}
            rx={rx}
            ry={ry}
            stroke={colorToCSS(item.lineColor)}
            strokeWidth={item.lineThickness ?? 1}
            fill={fill}
          />
        </g>
      );
    }
    case "Polygon": {
      if (item.points.length < 2) return null;
      const pts = item.points.map((point) => coordToSvg(point, cs, svgW, svgH));
      
      // Determine fill - gradient takes precedence over solid color
      let fill: string;
      if (item.fillGradient) {
        fill = `url(#gradient-${idx})`;
      } else if (isFilled(item.fillPattern)) {
        fill = colorToCSS(item.fillColor);
      } else {
        fill = "none";
      }
      
      return (
        <g key={idx} transform={transform}>
          <polygon
            points={pts.map((point) => `${point.x},${point.y}`).join(" ")}
            stroke={colorToCSS(item.lineColor)}
            strokeWidth={item.lineThickness ?? 1}
            fill={fill}
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

  // Collect gradient definitions from graphics
  const gradients = collectGradients(annotation.graphics);

  return (
    <svg
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      className={className ?? "block pointer-events-none"}
    >
      <defs>
        {gradients.map((grad, idx) => renderGradientDefinition(grad, idx))}
      </defs>
      {annotation.graphics.map((graphic, index) =>
        renderGraphicItem(graphic, index, annotation.coordinateSystem, width, height, instanceName),
      )}
      {selectedGraphic && renderSelection(selectedGraphic, annotation.coordinateSystem, width, height)}
    </svg>
  );
}

/**
 * Collect all gradient definitions from graphics
 */
function collectGradients(graphics: GraphicItem[]): Array<{ id: string; gradient: LinearGradient | RadialGradient; itemIndex: number }> {
  const gradients: Array<{ id: string; gradient: LinearGradient | RadialGradient; itemIndex: number }> = [];

  for (let idx = 0; idx < graphics.length; idx++) {
    const item = graphics[idx];
    if (item && "fillGradient" in item && item.fillGradient) {
      const id = `gradient-${idx}`;
      if (item.fillGradient.type === "linearGradient") {
        gradients.push({ id, gradient: item.fillGradient.gradient, itemIndex: idx });
      } else if (item.fillGradient.type === "radialGradient") {
        gradients.push({ id, gradient: item.fillGradient.gradient, itemIndex: idx });
      }
    }
  }

  return gradients;
}

/**
 * Render gradient definition as SVG def element
 */
function renderGradientDefinition(
  grad: { id: string; gradient: LinearGradient | RadialGradient },
  idx: number,
): React.ReactNode {
  const { id, gradient } = grad;

  if ("x1" in gradient) {
    // Linear gradient
    return (
      <linearGradient
        key={idx}
        id={id}
        x1={gradient.x1}
        y1={gradient.y1}
        x2={gradient.x2}
        y2={gradient.y2}
      >
        {gradient.stops.map((stop, i) => (
          <stop
            key={i}
            offset={`${stop.offset * 100}%`}
            stopColor={colorToCSS(stop.color)}
            stopOpacity={stop.opacity ?? 1}
          />
        ))}
      </linearGradient>
    );
  } else {
    // Radial gradient
    return (
      <radialGradient
        key={idx}
        id={id}
        cx={gradient.cx}
        cy={gradient.cy}
        r={gradient.r}
      >
        {gradient.stops.map((stop, i) => (
          <stop
            key={i}
            offset={`${stop.offset * 100}%`}
            stopColor={colorToCSS(stop.color)}
            stopOpacity={stop.opacity ?? 1}
          />
        ))}
      </radialGradient>
    );
  }
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
