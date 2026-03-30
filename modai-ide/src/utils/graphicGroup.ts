/**
 * Graphic grouping utilities
 */

import type {
  AnnotationPoint,
  GraphicBounds,
  GraphicGroup,
  GraphicItem,
} from "../components/diagramGraphicTypes";
import { getGraphicBounds, translateGraphicItem } from "../components/DiagramSvgRenderer";

/**
 * Group multiple top-level graphics into one Group item at the first selected index.
 */
export function groupGraphics(
  graphics: GraphicItem[],
  indices: number[],
): { updatedGraphics: GraphicItem[]; groupIndex: number } {
  if (indices.length < 2) {
    return { updatedGraphics: graphics, groupIndex: -1 };
  }

  const sortedIndices = [...indices].sort((a, b) => a - b);
  const selectedGraphics = sortedIndices
    .map((i) => graphics[i])
    .filter((g): g is GraphicItem => g !== undefined);

  if (selectedGraphics.length < 2) {
    return { updatedGraphics: graphics, groupIndex: -1 };
  }

  const next = [...graphics];
  for (let i = sortedIndices.length - 1; i >= 0; i--) {
    next.splice(sortedIndices[i]!, 1);
  }
  const insertAt = sortedIndices[0]!;
  const group: GraphicGroup = {
    type: "Group",
    children: selectedGraphics.map((g) => structuredClone(g) as GraphicItem),
  };
  next.splice(insertAt, 0, group);

  return {
    updatedGraphics: next,
    groupIndex: insertAt,
  };
}

/**
 * Replace a Group at index with its children (same z-order region).
 */
export function ungroupGraphics(graphics: GraphicItem[], index: number): GraphicItem[] {
  const item = graphics[index];
  if (!item || item.type !== "Group") return graphics;
  const next = [...graphics];
  next.splice(index, 1, ...item.children.map((c) => structuredClone(c) as GraphicItem));
  return next;
}

export function reorderGraphics(graphics: GraphicItem[], fromIndex: number, toIndex: number): GraphicItem[] {
  if (fromIndex === toIndex || fromIndex < 0 || toIndex < 0 || fromIndex >= graphics.length || toIndex >= graphics.length) {
    return graphics;
  }
  const next = [...graphics];
  const [moved] = next.splice(fromIndex, 1);
  if (!moved) return graphics;
  next.splice(toIndex, 0, moved);
  return next;
}

export function isPointInGraphic(point: AnnotationPoint, item: GraphicItem): boolean {
  const bounds = getGraphicBounds(item);
  if (!bounds) return false;
  return (
    point.x >= bounds.minX &&
    point.x <= bounds.maxX &&
    point.y >= bounds.minY &&
    point.y <= bounds.maxY
  );
}

export function findGraphicsAtPoint(graphics: GraphicItem[], point: AnnotationPoint): number[] {
  const indices: number[] = [];
  for (let i = 0; i < graphics.length; i++) {
    if (isPointInGraphic(point, graphics[i]!)) {
      indices.push(i);
    }
  }
  return indices;
}

export function moveGraphics(
  graphics: GraphicItem[],
  indices: number[],
  delta: AnnotationPoint,
): GraphicItem[] {
  const result = [...graphics];
  for (const index of indices) {
    if (result[index]) {
      result[index] = translateGraphicItem(result[index]!, delta);
    }
  }
  return result;
}

export function deleteGraphics(graphics: GraphicItem[], indices: number[]): GraphicItem[] {
  return graphics.filter((_, i) => !indices.includes(i));
}

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

  const boundsList = selectedGraphics
    .map((g) => getGraphicBounds(g))
    .filter((b): b is GraphicBounds => b !== null);

  const avgWidth =
    boundsList.length > 0 ?
      boundsList.reduce((sum, b) => sum + (b.maxX - b.minX), 0) / boundsList.length
    : 20;
  const offset: AnnotationPoint = {
    x: avgWidth * 0.2,
    y: -avgWidth * 0.2,
  };

  const duplicated = selectedGraphics.map((g) => translateGraphicItem(structuredClone(g) as GraphicItem, offset));

  const updatedGraphics = [...graphics, ...duplicated];
  const newIndices = duplicated.map((_, i) => graphics.length + i);

  return { updatedGraphics, newIndices };
}
