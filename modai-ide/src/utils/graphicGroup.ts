/**
 * Graphic grouping utilities
 * Provides functions to group and ungroup graphic items
 */

import type {
  GraphicItem,
  AnnotationPoint,
  GraphicBounds,
} from "../components/DiagramSvgRenderer";

/**
 * Group multiple graphics into a single group item
 */
export function groupGraphics(
  graphics: GraphicItem[],
  indices: number[],
): { updatedGraphics: GraphicItem[]; groupIndex: number } {
  if (indices.length < 2) {
    return { updatedGraphics: graphics, groupIndex: -1 };
  }

  // Get selected graphics sorted by index
  const sortedIndices = [...indices].sort((a, b) => a - b);
  const selectedGraphics = sortedIndices
    .map((i) => graphics[i])
    .filter((g): g is GraphicItem => g !== undefined);

  if (selectedGraphics.length < 2) {
    return { updatedGraphics: graphics, groupIndex: -1 };
  }

  // Calculate bounding box
  const boundsList = selectedGraphics
    .map((g) => getGraphicBounds(g))
    .filter((b): b is GraphicBounds => b !== null);

  if (boundsList.length === 0) {
    return { updatedGraphics: graphics, groupIndex: -1 };
  }

  const minX = Math.min(...boundsList.map((b) => b.minX));
  const minY = Math.min(...boundsList.map((b) => b.minY));
  const maxX = Math.max(...boundsList.map((b) => b.maxX));
  const maxY = Math.max(...boundsList.map((b) => b.maxY));

  // Group container would use this origin for future group operations
  // const _origin: AnnotationPoint = {
  //   x: (minX + maxX) / 2,
  //   y: (minY + maxY) / 2,
  // };

  // Create group item (using a Rectangle as visual container for now)
  // In a full implementation, this would be a proper Group type
  const groupContainer: GraphicItem = {
    type: "Rectangle",
    extent: {
      p1: { x: minX, y: maxY },
      p2: { x: maxX, y: minY },
    },
    lineColor: { r: 128, g: 128, b: 128 },
    lineThickness: 0.5,
    fillPattern: "none",
  };

  // Remove grouped items (in reverse order to maintain indices)
  const remainingGraphics = graphics.filter((_, i) => !indices.includes(i));
  
  // Add group container
  remainingGraphics.push(groupContainer);

  return {
    updatedGraphics: remainingGraphics,
    groupIndex: remainingGraphics.length - 1,
  };
}

/**
 * Ungroup a grouped item
 * Note: This is a placeholder for when proper Group type is implemented
 */
export function ungroupGraphics(
  graphics: GraphicItem[],
  index: number,
): GraphicItem[] {
  const item = graphics[index];
  if (!item) return graphics;

  // For now, just return the graphics as-is
  // In a full implementation, this would expand the group
  return graphics;
}

/**
 * Get bounding box for a graphic item
 */
function getGraphicBounds(item: GraphicItem): GraphicBounds | null {
  let points: AnnotationPoint[] = [];

  switch (item.type) {
    case "Line":
    case "Polygon":
    case "BSpline":
      points = item.points;
      break;
    case "Rectangle":
    case "Ellipse":
    case "Text":
    case "Bitmap":
      if (!item.extent) return null;
      points = [
        item.extent.p1,
        item.extent.p2,
        { x: item.extent.p1.x, y: item.extent.p2.y },
        { x: item.extent.p2.x, y: item.extent.p1.y },
      ];
      break;
    default:
      return null;
  }

  if (points.length === 0) return null;

  return {
    minX: Math.min(...points.map((p) => p.x)),
    minY: Math.min(...points.map((p) => p.y)),
    maxX: Math.max(...points.map((p) => p.x)),
    maxY: Math.max(...points.map((p) => p.y)),
  };
}

/**
 * Check if a point is inside a graphic item
 */
export function isPointInGraphic(
  point: AnnotationPoint,
  item: GraphicItem,
): boolean {
  const bounds = getGraphicBounds(item);
  if (!bounds) return false;

  return (
    point.x >= bounds.minX &&
    point.x <= bounds.maxX &&
    point.y >= bounds.minY &&
    point.y <= bounds.maxY
  );
}

/**
 * Get all graphics that contain a point
 */
export function findGraphicsAtPoint(
  graphics: GraphicItem[],
  point: AnnotationPoint,
): number[] {
  const indices: number[] = [];

  for (let i = 0; i < graphics.length; i++) {
    if (isPointInGraphic(point, graphics[i])) {
      indices.push(i);
    }
  }

  return indices;
}

/**
 * Move multiple graphics by a delta
 */
export function moveGraphics(
  graphics: GraphicItem[],
  indices: number[],
  delta: AnnotationPoint,
): GraphicItem[] {
  const result = [...graphics];

  for (const index of indices) {
    if (result[index]) {
      result[index] = translateGraphicItem(result[index], delta);
    }
  }

  return result;
}

/**
 * Translate a graphic item by a delta
 */
function translateGraphicItem(
  item: GraphicItem,
  delta: AnnotationPoint,
): GraphicItem {
  const movePoint = (point: AnnotationPoint) => ({
    x: point.x + delta.x,
    y: point.y + delta.y,
  });

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

/**
 * Delete multiple graphics
 */
export function deleteGraphics(
  graphics: GraphicItem[],
  indices: number[],
): GraphicItem[] {
  return graphics.filter((_, i) => !indices.includes(i));
}

/**
 * Duplicate multiple graphics
 */
export function duplicateGraphics(
  graphics: GraphicItem[],
  indices: number[],
): { updatedGraphics: GraphicItem[]; newIndices: number[] } {
  const sortedIndices = [...indices].sort((a, b) => a - b);
  const selectedGraphics = sortedIndices
    .map((i) => graphics[i])
    .filter((g): g is GraphicItem => g !== undefined);

  if (selectedGraphics.length === 0) {
    return { updatedGraphics: graphics, newIndices: [] };
  }

  // Calculate offset for duplication
  const boundsList = selectedGraphics
    .map((g) => getGraphicBounds(g))
    .filter((b): b is GraphicBounds => b !== null);

  const avgWidth =
    boundsList.reduce((sum, b) => sum + (b.maxX - b.minX), 0) / boundsList.length;
  const offset: AnnotationPoint = {
    x: avgWidth * 0.2,
    y: -avgWidth * 0.2,
  };

  // Duplicate and offset
  const duplicated = selectedGraphics.map((g) =>
    translateGraphicItem(g, offset),
  );

  // Add to graphics
  const updatedGraphics = [...graphics, ...duplicated];
  const newIndices = duplicated.map((_, i) => graphics.length + i);

  return { updatedGraphics, newIndices };
}
