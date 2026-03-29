/**
 * Alignment toolbar for graphic editing
 * Provides alignment and distribution tools for multiple selected graphics
 */

import {
  ArrowLeftToLine,
  ArrowLeftRight,
  ArrowRightToLine,
  ArrowUpToLine,
  ArrowUpDown,
  ArrowDownToLine,
  Split,
  Combine,
} from "lucide-react";
import type { GraphicItem } from "../DiagramSvgRenderer";
import { t } from "../../i18n";

export type AlignmentType = "left" | "center" | "right" | "top" | "middle" | "bottom";
export type DistributionType = "horizontal" | "vertical";

export interface AlignmentToolbarProps {
  selectedGraphics: GraphicItem[];
  selectedIndices: number[];
  onAlign: (alignment: AlignmentType) => void;
  onDistribute: (distribution: DistributionType) => void;
}

function ToolbarSeparator() {
  return <div className="w-px h-4 bg-[var(--border)]" />;
}

export function AlignmentToolbar({
  selectedGraphics,
  selectedIndices: _selectedIndices,
  onAlign,
  onDistribute,
}: AlignmentToolbarProps) {
  const hasSelection = selectedGraphics.length >= 2;

  if (!hasSelection) return null;

  return (
    <div className="flex items-center gap-0.5 rounded border border-[var(--border)] bg-[var(--surface)] p-0.5">
      <button
        type="button"
        className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
        onClick={() => onAlign("left")}
        title={t("alignLeft")}
      >
        <ArrowLeftToLine className="h-4 w-4" />
      </button>
      <button
        type="button"
        className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
        onClick={() => onAlign("center")}
        title={t("alignCenter")}
      >
        <ArrowLeftRight className="h-4 w-4" />
      </button>
      <button
        type="button"
        className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
        onClick={() => onAlign("right")}
        title={t("alignRight")}
      >
        <ArrowRightToLine className="h-4 w-4" />
      </button>
      <ToolbarSeparator />
      <button
        type="button"
        className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
        onClick={() => onAlign("top")}
        title={t("alignTop")}
      >
        <ArrowUpToLine className="h-4 w-4" />
      </button>
      <button
        type="button"
        className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
        onClick={() => onAlign("middle")}
        title={t("alignMiddle")}
      >
        <ArrowUpDown className="h-4 w-4" />
      </button>
      <button
        type="button"
        className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
        onClick={() => onAlign("bottom")}
        title={t("alignBottom")}
      >
        <ArrowDownToLine className="h-4 w-4" />
      </button>
      <ToolbarSeparator />
      <button
        type="button"
        className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
        onClick={() => onDistribute("horizontal")}
        title={t("distributeHorizontal")}
      >
        <Split className="h-4 w-4" />
      </button>
      <button
        type="button"
        className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10"
        onClick={() => onDistribute("vertical")}
        title={t("distributeVertical")}
      >
        <Combine className="h-4 w-4" />
      </button>
    </div>
  );
}

/**
 * Align graphics according to the specified alignment type
 */
export function alignGraphics(
  graphics: GraphicItem[],
  indices: number[],
  alignment: AlignmentType,
): GraphicItem[] {
  if (indices.length < 2) return graphics;

  const selectedGraphics = indices.map((i) => graphics[i]).filter((g): g is GraphicItem => g !== undefined);
  if (selectedGraphics.length < 2) return graphics;

  // Get bounds for all selected graphics
  const boundsList = selectedGraphics
    .map((g) => {
      const bounds = getGraphicBounds(g);
      return bounds;
    })
    .filter((b): b is GraphicBounds => b !== null);

  if (boundsList.length === 0) return graphics;

  let referenceValue: number;
  let resultGraphics = [...graphics];

  switch (alignment) {
    case "left":
      referenceValue = Math.min(...boundsList.map((b) => b.minX));
      indices.forEach((idx, i) => {
        const b = boundsList[i];
        if (!b) return;
        const delta = { x: referenceValue - b.minX, y: 0 };
        resultGraphics[idx] = translateGraphicItem(resultGraphics[idx], delta);
      });
      break;

    case "center": {
      const sumCenterX = boundsList.reduce((sum, b) => sum + (b.minX + b.maxX) / 2, 0);
      referenceValue = sumCenterX / boundsList.length;
      indices.forEach((idx, i) => {
        const b = boundsList[i];
        if (!b) return;
        const centerX = (b.minX + b.maxX) / 2;
        const delta = { x: referenceValue - centerX, y: 0 };
        resultGraphics[idx] = translateGraphicItem(resultGraphics[idx], delta);
      });
      break;
    }

    case "right":
      referenceValue = Math.max(...boundsList.map((b) => b.maxX));
      indices.forEach((idx, i) => {
        const b = boundsList[i];
        if (!b) return;
        const delta = { x: referenceValue - b.maxX, y: 0 };
        resultGraphics[idx] = translateGraphicItem(resultGraphics[idx], delta);
      });
      break;

    case "top":
      referenceValue = Math.max(...boundsList.map((b) => b.maxY));
      indices.forEach((idx, i) => {
        const b = boundsList[i];
        if (!b) return;
        const delta = { x: 0, y: referenceValue - b.maxY };
        resultGraphics[idx] = translateGraphicItem(resultGraphics[idx], delta);
      });
      break;

    case "middle": {
      const sumCenterY = boundsList.reduce((sum, b) => sum + (b.minY + b.maxY) / 2, 0);
      referenceValue = sumCenterY / boundsList.length;
      indices.forEach((idx, i) => {
        const b = boundsList[i];
        if (!b) return;
        const centerY = (b.minY + b.maxY) / 2;
        const delta = { x: 0, y: referenceValue - centerY };
        resultGraphics[idx] = translateGraphicItem(resultGraphics[idx], delta);
      });
      break;
    }

    case "bottom":
      referenceValue = Math.min(...boundsList.map((b) => b.minY));
      indices.forEach((idx, i) => {
        const b = boundsList[i];
        if (!b) return;
        const delta = { x: 0, y: referenceValue - b.minY };
        resultGraphics[idx] = translateGraphicItem(resultGraphics[idx], delta);
      });
      break;
  }

  return resultGraphics;
}

/**
 * Distribute graphics evenly
 */
export function distributeGraphics(
  graphics: GraphicItem[],
  indices: number[],
  distribution: DistributionType,
): GraphicItem[] {
  if (indices.length < 3) return graphics;

  const selectedGraphics = indices.map((i) => graphics[i]).filter((g): g is GraphicItem => g !== undefined);
  if (selectedGraphics.length < 3) return graphics;

  // Get bounds for all selected graphics
  const boundsList = selectedGraphics
    .map((g) => {
      const bounds = getGraphicBounds(g);
      return bounds;
    })
    .filter((b): b is GraphicBounds => b !== null);

  if (boundsList.length < 3) return graphics;

  let resultGraphics = [...graphics];

  if (distribution === "horizontal") {
    // Sort by x position
    const sortedIndices = indices
      .map((idx, i) => ({ idx, bounds: boundsList[i] }))
      .filter((item): item is { idx: number; bounds: GraphicBounds } => item.bounds !== null)
      .sort((a, b) => (a.bounds.minX + a.bounds.maxX) / 2 - (b.bounds.minX + b.bounds.maxX) / 2);

    const minX = sortedIndices[0].bounds.minX;
    const maxX = sortedIndices[sortedIndices.length - 1].bounds.maxX;
    const totalWidth = sortedIndices.reduce((sum, item) => sum + (item.bounds.maxX - item.bounds.minX), 0);
    const gapCount = sortedIndices.length - 1;
    const gapSize = (maxX - minX - totalWidth) / gapCount;

    let currentX = minX;
    sortedIndices.forEach((item, i) => {
      if (i === 0) return; // Skip first
      const width = item.bounds.maxX - item.bounds.minX;
      const targetX = currentX + gapSize;
      const delta = { x: targetX - item.bounds.minX, y: 0 };
      resultGraphics[item.idx] = translateGraphicItem(resultGraphics[item.idx], delta);
      currentX = targetX + width;
    });
  } else {
    // Sort by y position
    const sortedIndices = indices
      .map((idx, i) => ({ idx, bounds: boundsList[i] }))
      .filter((item): item is { idx: number; bounds: GraphicBounds } => item.bounds !== null)
      .sort((a, b) => (a.bounds.minY + a.bounds.maxY) / 2 - (b.bounds.minY + b.bounds.maxY) / 2);

    const minY = sortedIndices[0].bounds.minY;
    const maxY = sortedIndices[sortedIndices.length - 1].bounds.maxY;
    const totalHeight = sortedIndices.reduce((sum, item) => sum + (item.bounds.maxY - item.bounds.minY), 0);
    const gapCount = sortedIndices.length - 1;
    const gapSize = (maxY - minY - totalHeight) / gapCount;

    let currentY = minY;
    sortedIndices.forEach((item, i) => {
      if (i === 0) return; // Skip first
      const height = item.bounds.maxY - item.bounds.minY;
      const targetY = currentY + gapSize;
      const delta = { x: 0, y: targetY - item.bounds.minY };
      resultGraphics[item.idx] = translateGraphicItem(resultGraphics[item.idx], delta);
      currentY = targetY + height;
    });
  }

  return resultGraphics;
}

// Import required functions from DiagramSvgRenderer
function getGraphicBounds(item: GraphicItem): GraphicBounds | null {
  // Simplified implementation - should import from DiagramSvgRenderer
  let points: { x: number; y: number }[] = [];

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

interface GraphicBounds {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

function translateGraphicItem(item: GraphicItem, delta: { x: number; y: number }): GraphicItem {
  const movePoint = (point: { x: number; y: number }) => ({ x: point.x + delta.x, y: point.y + delta.y });

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
