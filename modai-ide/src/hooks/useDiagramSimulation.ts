import { useCallback, useMemo } from "react";
import type { UseStepDebugResult } from "./useStepDebug";

export interface SimOverlayData {
  nodeValues: Record<string, Record<string, number>>;
  edgeFlows: Record<string, number>;
}

function valueToColor(value: number, min: number, max: number): string {
  if (max === min) return "rgb(100,100,255)";
  const normalized = (value - min) / (max - min);
  const r = Math.round(normalized * 255);
  const b = Math.round((1 - normalized) * 255);
  return `rgb(${r}, 60, ${b})`;
}

export interface UseDiagramSimulationResult {
  overlayData: SimOverlayData | null;
  getNodeColor: (nodeName: string) => string | null;
  getNodeValues: (nodeName: string) => Record<string, number> | null;
  colorMap: Record<string, string>;
  isActive: boolean;
}

export function useDiagramSimulation(debug: UseStepDebugResult): UseDiagramSimulationResult {
  const { state } = debug;
  const currentStep = state.currentStep;
  const isActive = state.status !== "idle" && currentStep !== null;

  const overlayData = useMemo<SimOverlayData | null>(() => {
    if (!currentStep) return null;

    const nodeValues: Record<string, Record<string, number>> = {};
    const edgeFlows: Record<string, number> = {};

    for (let i = 0; i < currentStep.stateNames.length; i++) {
      const fullName = currentStep.stateNames[i];
      const parts = fullName.split(".");
      const nodeName = parts.length > 1 ? parts[0] : "__model__";
      const varName = parts.length > 1 ? parts.slice(1).join(".") : fullName;
      if (!nodeValues[nodeName]) nodeValues[nodeName] = {};
      nodeValues[nodeName][varName] = currentStep.states[i];
    }

    for (let i = 0; i < currentStep.outputNames.length; i++) {
      const fullName = currentStep.outputNames[i];
      const parts = fullName.split(".");
      const nodeName = parts.length > 1 ? parts[0] : "__model__";
      const varName = parts.length > 1 ? parts.slice(1).join(".") : fullName;
      if (!nodeValues[nodeName]) nodeValues[nodeName] = {};
      nodeValues[nodeName][varName] = currentStep.outputs[i];
    }

    return { nodeValues, edgeFlows };
  }, [currentStep]);

  const colorMap = useMemo<Record<string, string>>(() => {
    if (!overlayData) return {};
    const map: Record<string, string> = {};

    let globalMin = Infinity;
    let globalMax = -Infinity;
    for (const vals of Object.values(overlayData.nodeValues)) {
      for (const v of Object.values(vals)) {
        if (v < globalMin) globalMin = v;
        if (v > globalMax) globalMax = v;
      }
    }

    for (const [node, vals] of Object.entries(overlayData.nodeValues)) {
      const avgVal =
        Object.values(vals).reduce((s, v) => s + v, 0) /
        Math.max(1, Object.values(vals).length);
      map[node] = valueToColor(avgVal, globalMin, globalMax);
    }
    return map;
  }, [overlayData]);

  const getNodeColor = useCallback(
    (nodeName: string): string | null => {
      return colorMap[nodeName] ?? null;
    },
    [colorMap],
  );

  const getNodeValues = useCallback(
    (nodeName: string): Record<string, number> | null => {
      return overlayData?.nodeValues[nodeName] ?? null;
    },
    [overlayData],
  );

  return {
    overlayData,
    getNodeColor,
    getNodeValues,
    colorMap,
    isActive,
  };
}
