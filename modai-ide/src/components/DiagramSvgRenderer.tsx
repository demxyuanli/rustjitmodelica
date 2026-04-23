import React from "react";
import { createBSplinePath, renderArrowheads } from "./DiagramSvgArrows";
import { CONNECTOR_COLORS } from "./diagramConnectorColors";
import type {
  AnnotationExtent,
  AnnotationPoint,
  ConnectorAnchor,
  CoordinateSystem,
  GraphicBitmap,
  GraphicBSpline,
  GraphicBounds,
  GraphicEditHandle,
  GraphicEllipse,
  GraphicItem,
  GraphicLine,
  GraphicPolygon,
  GraphicRectangle,
  GraphicText,
  IconDiagramAnnotation,
  LinearGradient,
  RadialGradient,
} from "./diagramGraphicTypes";
import { DEFAULT_ICON_SIZE } from "./diagramGraphicTypes";
import { colorToCSS, patternStringToStrokeDasharray } from "./diagramSvg/diagramSvgStyle";

export * from "./diagramGraphicTypes";
export { colorToCSS, patternStringToStrokeDasharray } from "./diagramSvg/diagramSvgStyle";

function warnIfSlowDiagramRender(label: string, startMs: number) {
  if (typeof performance === "undefined") return;
  const dt = performance.now() - startMs;
  if (dt > 16) {
    console.warn(`[DiagramSvg] ${label} took ${dt.toFixed(2)}ms`);
  }
}

function clampGraphicOpacity(value?: number): number {
  if (value == null || Number.isNaN(value)) return 1;
  return Math.max(0, Math.min(1, value));
}

function graphicRotationCenterCoord(item: GraphicItem, bounds: GraphicBounds): AnnotationPoint {
  if ("origin" in item && item.origin) {
    return item.origin;
  }
  return { x: (bounds.minX + bounds.maxX) / 2, y: (bounds.minY + bounds.maxY) / 2 };
}

/** SVG transform for rotation (deg, Modelica CCW) and mirror around origin or bounds center. */
export function graphicOuterTransformSvg(
  item: GraphicItem,
  cs: CoordinateSystem | undefined,
  svgW: number,
  svgH: number,
): string | undefined {
  const bounds = getGraphicBounds(item);
  if (!bounds) return undefined;
  const centerCoord = graphicRotationCenterCoord(item, bounds);
  const centerSvg = coordToSvg(centerCoord, cs, svgW, svgH);
  const rotation = ("rotation" in item && item.rotation != null ? item.rotation : 0) as number;
  const mirrorX = ("mirrorX" in item && item.mirrorX) ?? false;
  const mirrorY = ("mirrorY" in item && item.mirrorY) ?? false;
  const sx = mirrorX ? -1 : 1;
  const sy = mirrorY ? -1 : 1;
  if (!rotation && !mirrorX && !mirrorY) return undefined;
  return `translate(${centerSvg.x},${centerSvg.y}) rotate(${-rotation}) scale(${sx},${sy}) translate(${-centerSvg.x},${-centerSvg.y})`;
}

function wrapGraphicNode(
  reactKey: string,
  item: GraphicItem,
  inner: React.ReactNode,
  cs: CoordinateSystem | undefined,
  svgW: number,
  svgH: number,
): React.ReactNode {
  const outerTransform = graphicOuterTransformSvg(item, cs, svgW, svgH);
  const opacity = clampGraphicOpacity("opacity" in item ? item.opacity : undefined);
  return (
    <g key={reactKey} transform={outerTransform} opacity={opacity}>
      {inner}
    </g>
  );
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
  if (item.type === "Group") {
    const boxes = item.children.map((c) => getGraphicBounds(c)).filter((b): b is GraphicBounds => b != null);
    if (boxes.length === 0) return null;
    return {
      minX: Math.min(...boxes.map((b) => b.minX)),
      minY: Math.min(...boxes.map((b) => b.minY)),
      maxX: Math.max(...boxes.map((b) => b.maxX)),
      maxY: Math.max(...boxes.map((b) => b.maxY)),
    };
  }
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
    case "Group":
      return {
        ...item,
        children: item.children.map((c) => translateGraphicItem(c, delta)),
        origin: item.origin ? movePoint(item.origin) : item.origin,
      };
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

/** Minimum width/height in model units when resizing extent-based graphics. */
export const EDIT_EXTENT_MIN_SPAN = 5;

export function inverseRotatePoint(point: AnnotationPoint, rotation = 0, origin?: AnnotationPoint): AnnotationPoint {
  if (!rotation) return point;
  const center = origin ?? { x: 0, y: 0 };
  const radians = (-rotation * Math.PI) / 180;
  const dx = point.x - center.x;
  const dy = point.y - center.y;
  return {
    x: center.x + dx * Math.cos(radians) - dy * Math.sin(radians),
    y: center.y + dx * Math.sin(radians) + dy * Math.cos(radians),
  };
}

/** Axis-aligned corners in local model space: top-left, top-right, bottom-right, bottom-left (Modelica Y up). */
export function localExtentCorners(ext: AnnotationExtent): [AnnotationPoint, AnnotationPoint, AnnotationPoint, AnnotationPoint] {
  const minX = Math.min(ext.p1.x, ext.p2.x);
  const maxX = Math.max(ext.p1.x, ext.p2.x);
  const minY = Math.min(ext.p1.y, ext.p2.y);
  const maxY = Math.max(ext.p1.y, ext.p2.y);
  return [
    { x: minX, y: maxY },
    { x: maxX, y: maxY },
    { x: maxX, y: minY },
    { x: minX, y: minY },
  ];
}

export function resizeExtentByLocalCorner(
  ext: AnnotationExtent,
  cornerIndex: number,
  newLocal: AnnotationPoint,
  minSpan: number,
): AnnotationExtent {
  const corners = localExtentCorners(ext);
  const fixed = corners[(cornerIndex + 2) % 4]!;
  let minX = Math.min(fixed.x, newLocal.x);
  let maxX = Math.max(fixed.x, newLocal.x);
  let minY = Math.min(fixed.y, newLocal.y);
  let maxY = Math.max(fixed.y, newLocal.y);
  if (maxX - minX < minSpan) {
    const mid = (minX + maxX) / 2;
    minX = mid - minSpan / 2;
    maxX = mid + minSpan / 2;
  }
  if (maxY - minY < minSpan) {
    const mid = (minY + maxY) / 2;
    minY = mid - minSpan / 2;
    maxY = mid + minSpan / 2;
  }
  return { p1: { x: minX, y: minY }, p2: { x: maxX, y: maxY } };
}

export function getEditHandleHitTolerance(cs?: CoordinateSystem): number {
  const ext = getCoordinateExtent(cs);
  const cw = Math.abs(ext.p2.x - ext.p1.x) || 200;
  const ch = Math.abs(ext.p2.y - ext.p1.y) || 200;
  return Math.max(10, Math.min(cw, ch) * 0.04);
}

export function listGraphicEditHandlesInWorld(
  item: GraphicItem,
): Array<{ handle: GraphicEditHandle; world: AnnotationPoint }> {
  if (item.type === "Group" || item.layerHidden) return [];
  const rot = "rotation" in item && item.rotation != null ? item.rotation : 0;
  const origin = "origin" in item ? item.origin : undefined;
  if (item.type === "Rectangle" || item.type === "Ellipse" || item.type === "Text" || item.type === "Bitmap") {
    if (!item.extent) return [];
    const corners = localExtentCorners(item.extent);
    return corners.map((local, cornerIndex) => ({
      handle: { kind: "extent-corner" as const, cornerIndex },
      world: rotatePoint(local, rot, origin),
    }));
  }
  if (item.type === "Line" || item.type === "Polygon" || item.type === "BSpline") {
    return item.points.map((local, pointIndex) => ({
      handle: { kind: "poly-point" as const, pointIndex },
      world: rotatePoint(local, rot, origin),
    }));
  }
  return [];
}

export function hitGraphicEditHandle(
  item: GraphicItem,
  worldPoint: AnnotationPoint,
  tolerance: number,
): GraphicEditHandle | null {
  const list = listGraphicEditHandlesInWorld(item);
  for (const { handle, world } of list) {
    if (pointDistance(worldPoint, world) <= tolerance) return handle;
  }
  return null;
}

function applyExtentCornerDrag(
  item: GraphicRectangle | GraphicEllipse | GraphicText | GraphicBitmap,
  cornerIndex: number,
  newWorld: AnnotationPoint,
  minSpan: number,
): GraphicRectangle | GraphicEllipse | GraphicText | GraphicBitmap {
  if (!item.extent) return item;
  const nl = inverseRotatePoint(newWorld, item.rotation ?? 0, item.origin);
  const nextExt = resizeExtentByLocalCorner(item.extent, cornerIndex, nl, minSpan);
  return { ...item, extent: nextExt };
}

function applyPolylinePointDrag(
  item: GraphicLine | GraphicPolygon | GraphicBSpline,
  pointIndex: number,
  newWorld: AnnotationPoint,
): GraphicLine | GraphicPolygon | GraphicBSpline {
  const nl = inverseRotatePoint(newWorld, item.rotation ?? 0, item.origin);
  const pts = [...item.points];
  if (pointIndex < 0 || pointIndex >= pts.length) return item;
  pts[pointIndex] = nl;
  return { ...item, points: pts };
}

export function applyGraphicEditHandleDrag(
  item: GraphicItem,
  handle: GraphicEditHandle,
  newWorld: AnnotationPoint,
  minSpan = EDIT_EXTENT_MIN_SPAN,
): GraphicItem {
  if (handle.kind === "extent-corner") {
    if (item.type === "Rectangle" || item.type === "Ellipse" || item.type === "Text" || item.type === "Bitmap") {
      return applyExtentCornerDrag(item, handle.cornerIndex, newWorld, minSpan);
    }
    return item;
  }
  if (handle.kind === "poly-point") {
    if (item.type === "Line" || item.type === "Polygon" || item.type === "BSpline") {
      return applyPolylinePointDrag(item, handle.pointIndex, newWorld);
    }
    return item;
  }
  return item;
}

export function rectangleToPolygonGraphic(item: GraphicRectangle): GraphicPolygon {
  const ext = item.extent ?? { p1: { x: -60, y: 40 }, p2: { x: 60, y: -40 } };
  const [tl, tr, br, bl] = localExtentCorners(ext);
  return {
    type: "Polygon",
    points: [tl, tr, br, bl],
    lineColor: item.lineColor,
    fillColor: item.fillColor,
    fillPattern: item.fillPattern,
    fillGradient: item.fillGradient,
    lineThickness: item.lineThickness,
    linePattern: item.borderPattern,
    rotation: item.rotation,
    origin: item.origin,
    opacity: item.opacity,
    mirrorX: item.mirrorX,
    mirrorY: item.mirrorY,
    layerHidden: item.layerHidden,
    layerLocked: item.layerLocked,
  };
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

function hitLeafGraphicItem(item: GraphicItem, point: AnnotationPoint, tolerance: number): boolean {
  if (item.layerHidden) return false;
  if (item.type === "Group") return false;
  if (item.type === "Line" || item.type === "Polygon" || item.type === "BSpline") {
    const points = transformPoints(item.points, item.rotation, item.origin);
    if (points.length < 2) return false;
    return points.some((current, currentIndex) => {
      if (currentIndex === 0) return false;
      return distanceToSegment(point, points[currentIndex - 1]!, current) <= tolerance;
    });
  }
  const bounds = getGraphicBounds(item);
  return !!(
    bounds &&
    point.x >= bounds.minX - tolerance &&
    point.x <= bounds.maxX + tolerance &&
    point.y >= bounds.minY - tolerance &&
    point.y <= bounds.maxY + tolerance
  );
}

function hitTestGraphicItem(item: GraphicItem, point: AnnotationPoint, tolerance: number): boolean {
  if (item.layerHidden) return false;
  if (item.type === "Group") {
    for (let ci = item.children.length - 1; ci >= 0; ci -= 1) {
      if (hitTestGraphicItem(item.children[ci]!, point, tolerance)) return true;
    }
    return false;
  }
  return hitLeafGraphicItem(item, point, tolerance);
}

function hitDeepestPathFromItem(item: GraphicItem, point: AnnotationPoint, tolerance: number): number[] | null {
  if (item.layerHidden) return null;
  if (item.type === "Group") {
    for (let ci = item.children.length - 1; ci >= 0; ci -= 1) {
      const ch = item.children[ci]!;
      if (ch.layerHidden) continue;
      const sub = hitDeepestPathFromItem(ch, point, tolerance);
      if (sub !== null) return [ci, ...sub];
    }
    const b = getGraphicBounds(item);
    if (
      b &&
      point.x >= b.minX - tolerance &&
      point.x <= b.maxX + tolerance &&
      point.y >= b.minY - tolerance &&
      point.y <= b.maxY + tolerance
    ) {
      return [];
    }
    return null;
  }
  if (hitLeafGraphicItem(item, point, tolerance)) return [];
  return null;
}

/**
 * Deepest hit path: [rootIndex] or [rootIndex, childIndex, ...] inside nested Groups.
 */
export function findDeepestGraphicPath(
  graphics: GraphicItem[],
  point: AnnotationPoint,
  tolerance = 8,
): number[] | null {
  for (let i = graphics.length - 1; i >= 0; i -= 1) {
    const item = graphics[i];
    if (!item || item.layerHidden) continue;
    const suffix = hitDeepestPathFromItem(item, point, tolerance);
    if (suffix !== null) return [i, ...suffix];
  }
  return null;
}

export function getGraphicAtPath(graphics: GraphicItem[], path: number[]): GraphicItem | null {
  if (path.length === 0) return null;
  let cur: GraphicItem | undefined = graphics[path[0]!];
  if (!cur) return null;
  for (let d = 1; d < path.length; d++) {
    if (cur.type !== "Group") return null;
    cur = cur.children[path[d]!];
    if (!cur) return null;
  }
  return cur;
}

export function replaceGraphicAtPath(graphics: GraphicItem[], path: number[], next: GraphicItem): GraphicItem[] {
  if (path.length === 0) return graphics;
  const root = path[0]!;
  if (root < 0 || root >= graphics.length) return graphics;
  if (path.length === 1) {
    const copy = [...graphics];
    copy[root] = next;
    return copy;
  }
  const item = graphics[root];
  if (!item || item.type !== "Group") return graphics;
  const newChildren = replaceGraphicAtPath(item.children, path.slice(1), next);
  const copy = [...graphics];
  copy[root] = { ...item, children: newChildren };
  return copy;
}

export function removeGraphicAtPath(graphics: GraphicItem[], path: number[]): GraphicItem[] | null {
  if (path.length === 0) return null;
  const root = path[0]!;
  if (root < 0 || root >= graphics.length) return null;
  if (path.length === 1) {
    return graphics.filter((_, i) => i !== root);
  }
  const item = graphics[root];
  if (!item || item.type !== "Group") return null;
  const newChildren = removeGraphicAtPath(item.children, path.slice(1));
  if (newChildren === null) return null;
  const copy = [...graphics];
  copy[root] = { ...item, children: newChildren };
  return copy;
}

export function findGraphicAtPoint(
  graphics: GraphicItem[],
  point: AnnotationPoint,
  tolerance = 8,
): number {
  for (let index = graphics.length - 1; index >= 0; index -= 1) {
    const item = graphics[index];
    if (!item || item.layerHidden) continue;
    if (hitTestGraphicItem(item, point, tolerance)) return index;
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
  pathId: string,
  cs?: CoordinateSystem,
  svgW = DEFAULT_ICON_SIZE,
  svgH = DEFAULT_ICON_SIZE,
  instanceName?: string,
): React.ReactNode {
  if (item.layerHidden) return null;
  switch (item.type) {
    case "Line": {
      if (item.points.length < 2) return null;
      const pts = item.points.map((point) => coordToSvg(point, cs, svgW, svgH));
      const d = pts.map((point, pointIndex) => `${pointIndex === 0 ? "M" : "L"} ${point.x} ${point.y}`).join(" ");
      const strokeDasharray = patternStringToStrokeDasharray(item.pattern);
      return wrapGraphicNode(
        pathId,
        item,
        <>
          <path
            d={d}
            stroke={colorToCSS(item.color)}
            strokeWidth={item.thickness ?? 1}
            fill="none"
            strokeDasharray={strokeDasharray}
          />
          {item.arrow && renderArrowheads(pts, item.arrow, item.color, item.arrowSize)}
        </>,
        cs,
        svgW,
        svgH,
      );
    }
    case "BSpline": {
      if (item.points.length < 2) return null;
      const pts = item.points.map((point) => coordToSvg(point, cs, svgW, svgH));
      const smooth = item.smooth === "BSpline" || item.smooth === "true";
      const d = smooth ? createBSplinePath(pts) : pts.map((point, pointIndex) => `${pointIndex === 0 ? "M" : "L"} ${point.x} ${point.y}`).join(" ");
      const strokeDasharray = patternStringToStrokeDasharray(item.pattern);
      return wrapGraphicNode(
        pathId,
        item,
        <>
          <path
            d={d}
            stroke={colorToCSS(item.color)}
            strokeWidth={item.thickness ?? 1}
            fill="none"
            strokeDasharray={strokeDasharray}
          />
          {item.arrow && renderArrowheads(pts, item.arrow, item.color, item.arrowSize)}
        </>,
        cs,
        svgW,
        svgH,
      );
    }
    case "Rectangle": {
      if (!item.extent) return null;
      const rect = extentToSvgRect(item.extent, cs, svgW, svgH);
      
      // Determine fill - gradient takes precedence over solid color
      let fill: string;
      if (item.fillGradient) {
        fill = `url(#gradient-${pathId})`;
      } else if (isFilled(item.fillPattern)) {
        fill = colorToCSS(item.fillColor);
      } else {
        fill = "none";
      }
      
      const strokeDasharray = patternStringToStrokeDasharray(item.borderPattern);
      return wrapGraphicNode(
        pathId,
        item,
        <rect
          x={rect.x}
          y={rect.y}
          width={rect.width}
          height={rect.height}
          rx={item.radius ?? 0}
          stroke={colorToCSS(item.lineColor)}
          strokeWidth={item.lineThickness ?? 1}
          fill={fill}
          strokeDasharray={strokeDasharray}
        />,
        cs,
        svgW,
        svgH,
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
        fill = `url(#gradient-${pathId})`;
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
        const sweep = svgEndRad > svgStartRad ? 1 : 0;

        // 扇形路径：从中心到起点，沿弧到终点，回到中心
        const d = `M ${cx} ${cy} L ${x1} ${y1} A ${rx} ${ry} 0 ${largeArc} ${sweep} ${x2} ${y2} Z`;
        const strokeDasharray = patternStringToStrokeDasharray(item.linePattern);
        return wrapGraphicNode(
          pathId,
          item,
          <path
            d={d}
            stroke={colorToCSS(item.lineColor)}
            strokeWidth={item.lineThickness ?? 1}
            fill={fill}
            strokeDasharray={strokeDasharray}
          />,
          cs,
          svgW,
          svgH,
        );
      }

      const strokeDasharray = patternStringToStrokeDasharray(item.linePattern);
      return wrapGraphicNode(
        pathId,
        item,
        <ellipse
          cx={cx}
          cy={cy}
          rx={rx}
          ry={ry}
          stroke={colorToCSS(item.lineColor)}
          strokeWidth={item.lineThickness ?? 1}
          fill={fill}
          strokeDasharray={strokeDasharray}
        />,
        cs,
        svgW,
        svgH,
      );
    }
    case "Polygon": {
      if (item.points.length < 2) return null;
      const pts = item.points.map((point) => coordToSvg(point, cs, svgW, svgH));
      
      // Determine fill - gradient takes precedence over solid color
      let fill: string;
      if (item.fillGradient) {
        fill = `url(#gradient-${pathId})`;
      } else if (isFilled(item.fillPattern)) {
        fill = colorToCSS(item.fillColor);
      } else {
        fill = "none";
      }
      
      const strokeDasharray = patternStringToStrokeDasharray(item.linePattern);
      return wrapGraphicNode(
        pathId,
        item,
        <polygon
          points={pts.map((point) => `${point.x},${point.y}`).join(" ")}
          stroke={colorToCSS(item.lineColor)}
          strokeWidth={item.lineThickness ?? 1}
          fill={fill}
          strokeDasharray={strokeDasharray}
        />,
        cs,
        svgW,
        svgH,
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
      return wrapGraphicNode(
        pathId,
        item,
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
        </text>,
        cs,
        svgW,
        svgH,
      );
    }
    case "Bitmap": {
      if (!item.extent) return null;
      const rect = extentToSvgRect(item.extent, cs, svgW, svgH);
      const href = item.fileName || item.imageSource;
      if (!href) return null;
      return wrapGraphicNode(
        pathId,
        item,
        <image href={href} x={rect.x} y={rect.y} width={rect.width} height={rect.height} preserveAspectRatio="none" />,
        cs,
        svgW,
        svgH,
      );
    }
    case "Group": {
      return wrapGraphicNode(
        pathId,
        item,
        <>
          {item.children.map((child, j) =>
            renderGraphicItem(child, `${pathId}-${j}`, cs, svgW, svgH, instanceName),
          )}
        </>,
        cs,
        svgW,
        svgH,
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
  const t0 = typeof performance !== "undefined" ? performance.now() : 0;
  const transform = rotation ? `rotate(${rotation} ${size / 2} ${size / 2})` : undefined;
  const gradients = collectGradients(icon.graphics);
  const el = (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} className="block">
      <defs>
        {gradients.map((grad) => renderGradientDefinition(grad))}
      </defs>
      <g transform={transform}>
        {icon.graphics.map((graphic, index) =>
          renderGraphicItem(graphic, String(index), icon.coordinateSystem, size, size, instanceName),
        )}
      </g>
    </svg>
  );
  warnIfSlowDiagramRender("IconSvg", t0);
  return el;
}

export function AnnotationGraphicsSvg({
  annotation,
  size,
  instanceName,
  selectedGraphicPath = null,
  selectedGraphicIndex = -1,
  className,
}: {
  annotation: IconDiagramAnnotation;
  size: { width: number; height: number };
  instanceName?: string;
  /** Preferred: nested selection [root, child, ...]. */
  selectedGraphicPath?: number[] | null;
  /** Fallback when path is not provided. */
  selectedGraphicIndex?: number;
  className?: string;
}) {
  const t0 = typeof performance !== "undefined" ? performance.now() : 0;
  const width = Math.max(1, size.width);
  const height = Math.max(1, size.height);
  const selectedGraphic =
    selectedGraphicPath != null && selectedGraphicPath.length > 0
      ? getGraphicAtPath(annotation.graphics, selectedGraphicPath)
      : selectedGraphicIndex >= 0
        ? annotation.graphics[selectedGraphicIndex]
        : null;

  // Collect gradient definitions from graphics
  const gradients = collectGradients(annotation.graphics);

  const el = (
    <svg
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      className={className ?? "block pointer-events-none"}
    >
      <defs>
        {gradients.map((grad) => renderGradientDefinition(grad))}
      </defs>
      {annotation.graphics.map((graphic, index) =>
        renderGraphicItem(graphic, String(index), annotation.coordinateSystem, width, height, instanceName),
      )}
      {selectedGraphic && renderSelection(selectedGraphic, annotation.coordinateSystem, width, height)}
    </svg>
  );
  warnIfSlowDiagramRender("AnnotationGraphicsSvg", t0);
  return el;
}

/**
 * Collect all gradient definitions from graphics (nested paths for groups).
 */
function collectGradients(graphics: GraphicItem[]): Array<{ id: string; gradient: LinearGradient | RadialGradient }> {
  const gradients: Array<{ id: string; gradient: LinearGradient | RadialGradient }> = [];

  function walk(items: GraphicItem[], prefix: string) {
    items.forEach((item, i) => {
      const path = prefix === "" ? `${i}` : `${prefix}-${i}`;
      if (item.type === "Group") {
        walk(item.children, path);
      } else if ("fillGradient" in item && item.fillGradient) {
        const id = `gradient-${path}`;
        if (item.fillGradient.type === "linearGradient") {
          gradients.push({ id, gradient: item.fillGradient.gradient });
        } else if (item.fillGradient.type === "radialGradient") {
          gradients.push({ id, gradient: item.fillGradient.gradient });
        }
      }
    });
  }

  walk(graphics, "");
  return gradients;
}

/**
 * Render gradient definition as SVG def element
 */
function renderGradientDefinition(grad: { id: string; gradient: LinearGradient | RadialGradient }): React.ReactNode {
  const { id, gradient } = grad;

  if ("x1" in gradient) {
    // Linear gradient
    return (
      <linearGradient
        key={id}
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
        key={id}
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

export { CONNECTOR_COLORS };

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
