import { useCallback, useState } from "react";
import type { GraphicItem } from "../components/diagramGraphicTypes";
import {
  getGraphicAtPath,
  removeGraphicAtPath,
  replaceGraphicAtPath,
  rectangleToPolygonGraphic,
} from "../components/DiagramSvgRenderer";
import { alignGraphics, distributeGraphics } from "../components/diagram/AlignmentToolbar";
import { duplicateGraphics, deleteGraphics, groupGraphics, ungroupGraphics, reorderGraphics } from "../utils/graphicGroup";

export function useDiagramGraphicInteraction(
  mode: "diagram" | "icon",
  activeGraphics: GraphicItem[],
  setGraphicsForActiveMode: (graphics: GraphicItem[]) => void,
) {
  const [selectedGraphicPath, setSelectedGraphicPath] = useState<number[] | null>(null);
  const [selectedGraphicIndices, setSelectedGraphicIndices] = useState<number[]>([]);

  const handleUpdateGraphic = useCallback(
    (path: number[], next: GraphicItem) => {
      setGraphicsForActiveMode(replaceGraphicAtPath(activeGraphics, path, next));
    },
    [activeGraphics, setGraphicsForActiveMode],
  );

  const handleRectangleToPolygon = useCallback(() => {
    if (!selectedGraphicPath?.length) return;
    const g = getGraphicAtPath(activeGraphics, selectedGraphicPath);
    if (!g || g.type !== "Rectangle") return;
    handleUpdateGraphic(selectedGraphicPath, rectangleToPolygonGraphic(g));
  }, [activeGraphics, selectedGraphicPath, handleUpdateGraphic]);

  const handleAddGraphic = useCallback(
    (graphic: GraphicItem) => {
      const graphics = [...activeGraphics, graphic];
      setGraphicsForActiveMode(graphics);
      const last = graphics.length - 1;
      setSelectedGraphicPath([last]);
      setSelectedGraphicIndices([last]);
    },
    [activeGraphics, setGraphicsForActiveMode],
  );

  const handleDeleteGraphic = useCallback(
    (path: number[]) => {
      if (path.length === 0) return;
      const updated = removeGraphicAtPath(activeGraphics, path);
      if (updated === null) return;
      setGraphicsForActiveMode(updated);
      const root = path[0]!;
      if (path.length === 1) {
        setSelectedGraphicIndices((prev) =>
          prev.filter((i) => i !== root).map((i) => (i > root ? i - 1 : i)),
        );
        setSelectedGraphicPath(null);
      } else {
        setSelectedGraphicPath(path.slice(0, -1));
      }
    },
    [activeGraphics, setGraphicsForActiveMode],
  );

  const handleSelectGraphic = useCallback(
    (path: number[] | null, additive?: boolean) => {
      if (mode !== "icon") {
        if (!path || path.length === 0) setSelectedGraphicPath(null);
        else setSelectedGraphicPath([path[0]!]);
        return;
      }
      if (!path || path.length === 0) {
        if (!additive) {
          setSelectedGraphicPath(null);
          setSelectedGraphicIndices([]);
        }
        return;
      }
      const root = path[0]!;
      if (additive) {
        const s = new Set(selectedGraphicIndices);
        const had = s.has(root);
        if (had) s.delete(root);
        else s.add(root);
        const next = [...s].sort((a, b) => a - b);
        setSelectedGraphicIndices(next);
        if (next.length === 0) setSelectedGraphicPath(null);
        else if (!had) setSelectedGraphicPath(path);
        else setSelectedGraphicPath(next[0] !== undefined ? [next[0]] : null);
        return;
      }
      setSelectedGraphicPath(path);
      setSelectedGraphicIndices([root]);
    },
    [mode, selectedGraphicIndices],
  );

  const patchGraphicAtPath = useCallback(
    (path: number[], patch: Partial<GraphicItem>) => {
      const g = getGraphicAtPath(activeGraphics, path);
      if (!g) return;
      const next = { ...structuredClone(g), ...patch } as GraphicItem;
      setGraphicsForActiveMode(replaceGraphicAtPath(activeGraphics, path, next));
    },
    [activeGraphics, setGraphicsForActiveMode],
  );

  const handleToggleLayerHidden = useCallback(
    (index: number) => {
      const g = activeGraphics[index];
      if (!g) return;
      patchGraphicAtPath([index], { layerHidden: !g.layerHidden });
    },
    [activeGraphics, patchGraphicAtPath],
  );

  const handleToggleLayerLocked = useCallback(
    (index: number) => {
      const g = activeGraphics[index];
      if (!g) return;
      patchGraphicAtPath([index], { layerLocked: !g.layerLocked });
    },
    [activeGraphics, patchGraphicAtPath],
  );

  const handleReorderGraphics = useCallback(
    (from: number, to: number) => {
      const updated = reorderGraphics(activeGraphics, from, to);
      setGraphicsForActiveMode(updated);
      const mapIndex = (i: number) => {
        if (i === from) return to;
        if (from < to) {
          if (i > from && i <= to) return i - 1;
        } else if (from > to) {
          if (i >= to && i < from) return i + 1;
        }
        return i;
      };
      setSelectedGraphicIndices((prev) => [...new Set(prev.map(mapIndex))].sort((a, b) => a - b));
      setSelectedGraphicPath((cur) => {
        if (!cur || cur.length === 0) return cur;
        const newRoot = mapIndex(cur[0]!);
        return [newRoot, ...cur.slice(1)];
      });
    },
    [activeGraphics, setGraphicsForActiveMode],
  );

  const handleGroupGraphics = useCallback(() => {
    if (selectedGraphicIndices.length < 2) return;
    const { updatedGraphics, groupIndex } = groupGraphics(activeGraphics, selectedGraphicIndices);
    if (groupIndex < 0) return;
    setGraphicsForActiveMode(updatedGraphics);
    setSelectedGraphicIndices([groupIndex]);
    setSelectedGraphicPath([groupIndex]);
  }, [activeGraphics, selectedGraphicIndices, setGraphicsForActiveMode]);

  const handleUngroupGraphics = useCallback(() => {
    if (selectedGraphicIndices.length !== 1) return;
    const idx = selectedGraphicIndices[0]!;
    const item = activeGraphics[idx];
    if (!item || item.type !== "Group") return;
    const n = item.children.length;
    const updated = ungroupGraphics(activeGraphics, idx);
    setGraphicsForActiveMode(updated);
    setSelectedGraphicIndices(Array.from({ length: n }, (_, i) => idx + i));
    setSelectedGraphicPath([idx]);
  }, [activeGraphics, selectedGraphicIndices, setGraphicsForActiveMode]);

  const handleAlign = useCallback(
    (alignment: "left" | "center" | "right" | "top" | "middle" | "bottom") => {
      if (selectedGraphicIndices.length < 2) return;
      const updated = alignGraphics(activeGraphics, selectedGraphicIndices, alignment);
      setGraphicsForActiveMode(updated);
      setSelectedGraphicIndices([]);
      setSelectedGraphicPath(null);
    },
    [activeGraphics, selectedGraphicIndices, setGraphicsForActiveMode],
  );

  const handleDistribute = useCallback(
    (distribution: "horizontal" | "vertical") => {
      if (selectedGraphicIndices.length < 3) return;
      const updated = distributeGraphics(activeGraphics, selectedGraphicIndices, distribution);
      setGraphicsForActiveMode(updated);
      setSelectedGraphicIndices([]);
      setSelectedGraphicPath(null);
    },
    [activeGraphics, selectedGraphicIndices, setGraphicsForActiveMode],
  );

  const handleDuplicate = useCallback(() => {
    if (selectedGraphicIndices.length === 0) return;
    const { updatedGraphics } = duplicateGraphics(activeGraphics, selectedGraphicIndices);
    setGraphicsForActiveMode(updatedGraphics);
    setSelectedGraphicIndices([]);
    setSelectedGraphicPath(null);
  }, [activeGraphics, selectedGraphicIndices, setGraphicsForActiveMode]);

  const handleDeleteSelected = useCallback(() => {
    if (selectedGraphicIndices.length === 0) return;
    const updated = deleteGraphics(activeGraphics, selectedGraphicIndices);
    setGraphicsForActiveMode(updated);
    setSelectedGraphicIndices([]);
    setSelectedGraphicPath(null);
  }, [activeGraphics, selectedGraphicIndices, setGraphicsForActiveMode]);

  return {
    selectedGraphicPath,
    setSelectedGraphicPath,
    selectedGraphicIndices,
    setSelectedGraphicIndices,
    handleUpdateGraphic,
    handleRectangleToPolygon,
    handleAddGraphic,
    handleDeleteGraphic,
    handleSelectGraphic,
    patchGraphicAtPath,
    handleToggleLayerHidden,
    handleToggleLayerLocked,
    handleReorderGraphics,
    handleGroupGraphics,
    handleUngroupGraphics,
    handleAlign,
    handleDistribute,
    handleDuplicate,
    handleDeleteSelected,
  };
}
