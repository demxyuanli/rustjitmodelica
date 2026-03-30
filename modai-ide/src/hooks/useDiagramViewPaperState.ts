import { useState } from "react";
import type { JointPaperHandle } from "../utils/jointUtils";

export function useDiagramViewPaperState() {
  const [paperHandle, setPaperHandle] = useState<JointPaperHandle | null>(null);
  const [showMiniMap, setShowMiniMap] = useState(true);
  const [gridEnabled, setGridEnabled] = useState(true);
  const [gridSize, setGridSize] = useState(10);
  const [showGrid, setShowGrid] = useState(true);

  return {
    paperHandle,
    setPaperHandle,
    showMiniMap,
    setShowMiniMap,
    gridEnabled,
    setGridEnabled,
    gridSize,
    setGridSize,
    showGrid,
    setShowGrid,
  };
}
