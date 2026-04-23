/**
 * Grid snapping utilities for graphic editing
 * Provides functions to snap points and extents to a grid
 */

import type { AnnotationExtent, AnnotationPoint } from "../components/diagramGraphicTypes";

export interface GridOptions {
  enabled: boolean;
  gridSize: number;
  snapTolerance: number;
}

export const DEFAULT_GRID_OPTIONS: GridOptions = {
  enabled: true,
  gridSize: 10,
  snapTolerance: 5,
};

/**
 * Snap a single point to the grid
 */
export function snapToGrid(point: AnnotationPoint, options: GridOptions = DEFAULT_GRID_OPTIONS): AnnotationPoint {
  if (!options.enabled) return point;

  const { gridSize, snapTolerance } = options;

  const snapX = Math.round(point.x / gridSize) * gridSize;
  const snapY = Math.round(point.y / gridSize) * gridSize;

  // Check if within snap tolerance
  const shouldSnapX = Math.abs(point.x - snapX) <= snapTolerance;
  const shouldSnapY = Math.abs(point.y - snapY) <= snapTolerance;

  return {
    x: shouldSnapX ? snapX : point.x,
    y: shouldSnapY ? snapY : point.y,
  };
}

/** Always align coordinates to the nearest grid intersection (structure diagram drag). */
export function snapPointToGridStrict(point: AnnotationPoint, gridSize: number): AnnotationPoint {
  if (gridSize <= 0) return point;
  return {
    x: Math.round(point.x / gridSize) * gridSize,
    y: Math.round(point.y / gridSize) * gridSize,
  };
}

/**
 * Snap an extent to the grid
 */
export function snapExtentToGrid(extent: AnnotationExtent, options: GridOptions = DEFAULT_GRID_OPTIONS): AnnotationExtent {
  return {
    p1: snapToGrid(extent.p1, options),
    p2: snapToGrid(extent.p2, options),
  };
}

/**
 * Snap multiple points to the grid
 */
export function snapPointsToGrid(points: AnnotationPoint[], options: GridOptions = DEFAULT_GRID_OPTIONS): AnnotationPoint[] {
  return points.map(p => snapToGrid(p, options));
}

/**
 * Calculate offset for grid-aligned movement
 */
export function getGridAlignedDelta(delta: AnnotationPoint, options: GridOptions = DEFAULT_GRID_OPTIONS): AnnotationPoint {
  if (!options.enabled) return delta;

  const { gridSize } = options;

  return {
    x: Math.round(delta.x / gridSize) * gridSize,
    y: Math.round(delta.y / gridSize) * gridSize,
  };
}

/**
 * Check if a point is near a grid intersection
 */
export function isNearGridIntersection(
  point: AnnotationPoint,
  gridReference: AnnotationPoint,
  options: GridOptions = DEFAULT_GRID_OPTIONS,
): boolean {
  const { gridSize, snapTolerance } = options;

  const dx = Math.abs(point.x - gridReference.x);
  const dy = Math.abs(point.y - gridReference.y);

  const nearGridX = dx % gridSize < snapTolerance || (gridSize - dx % gridSize) < snapTolerance;
  const nearGridY = dy % gridSize < snapTolerance || (gridSize - dy % gridSize) < snapTolerance;

  return nearGridX && nearGridY;
}

/**
 * Get grid line positions for rendering
 */
export function getGridLines(
  extent: AnnotationExtent,
  gridSize: number = DEFAULT_GRID_OPTIONS.gridSize,
): { x: number; y: number; isMajor: boolean }[] {
  const lines: { x: number; y: number; isMajor: boolean }[] = [];
  const minX = Math.min(extent.p1.x, extent.p2.x);
  const maxX = Math.max(extent.p1.x, extent.p2.x);
  const minY = Math.min(extent.p1.y, extent.p2.y);
  const maxY = Math.max(extent.p1.y, extent.p2.y);

  const majorGridInterval = gridSize * 5;

  // Vertical lines
  for (let x = Math.floor(minX / gridSize) * gridSize; x <= maxX; x += gridSize) {
    lines.push({
      x,
      y: 0, // Will be transformed during rendering
      isMajor: x % majorGridInterval === 0,
    });
  }

  // Horizontal lines
  for (let y = Math.floor(minY / gridSize) * gridSize; y <= maxY; y += gridSize) {
    lines.push({
      x: 0, // Will be transformed during rendering
      y,
      isMajor: y % majorGridInterval === 0,
    });
  }

  return lines;
}
