import { useState, useCallback } from "react";
import { ZoomIn, ZoomOut, Maximize2, X } from "lucide-react";
import { t } from "../i18n";
import {
  EquationGraphView,
  type LayoutAlgorithm,
  type LayoutDirection,
  type EquationGraphLayoutOptions,
} from "./EquationGraphView";
import type { JointPaperHandle } from "../utils/jointUtils";

export interface DependencyGraphModalProps {
  open: boolean;
  onClose: () => void;
  code: string;
  modelName: string;
  projectDir: string | null | undefined;
}

const ALGORITHMS: { value: LayoutAlgorithm; labelKey: string }[] = [
  { value: "layered", labelKey: "layoutLayered" },
  { value: "box", labelKey: "layoutBox" },
  { value: "force", labelKey: "layoutForce" },
];

const DIRECTIONS: { value: LayoutDirection; labelKey: string }[] = [
  { value: "RIGHT", labelKey: "directionRight" },
  { value: "DOWN", labelKey: "directionDown" },
  { value: "LEFT", labelKey: "directionLeft" },
  { value: "UP", labelKey: "directionUp" },
];

export function DependencyGraphModal({
  open,
  onClose,
  code,
  modelName,
  projectDir,
}: DependencyGraphModalProps) {
  const [algorithm, setAlgorithm] = useState<LayoutAlgorithm>("layered");
  const [direction, setDirection] = useState<LayoutDirection>("RIGHT");
  const [paperHandle, setPaperHandle] = useState<JointPaperHandle | null>(null);

  const layoutOptions: EquationGraphLayoutOptions = { algorithm, direction };

  const handleZoomIn = useCallback(() => {
    paperHandle?.zoomIn();
  }, [paperHandle]);

  const handleZoomOut = useCallback(() => {
    paperHandle?.zoomOut();
  }, [paperHandle]);

  const handleFitView = useCallback(() => {
    paperHandle?.fitView({ padding: 0.16 });
  }, [paperHandle]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center bg-[var(--surface-elevated)]"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className="flex flex-col border border-[var(--border)] rounded-lg shadow-xl w-[90vw] max-w-[1400px] h-[85vh] overflow-hidden bg-[var(--surface-elevated)]"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="shrink-0 flex items-center gap-2 px-3 py-1.5 border-b border-[var(--border)]">
          <h2 className="text-xs font-semibold text-[var(--text)] mr-auto">
            {t("dependencyGraphTitle")}
          </h2>
          <select
            className="rounded border border-[var(--border)] bg-[var(--bg-input)] text-[var(--text)] text-[10px] px-1.5 py-0.5"
            value={algorithm}
            onChange={(e) => setAlgorithm(e.target.value as LayoutAlgorithm)}
            title={t("layoutAlgorithm")}
          >
            {ALGORITHMS.map((a) => (
              <option key={a.value} value={a.value}>{t(a.labelKey)}</option>
            ))}
          </select>
          <select
            className="rounded border border-[var(--border)] bg-[var(--bg-input)] text-[var(--text)] text-[10px] px-1.5 py-0.5"
            value={direction}
            onChange={(e) => setDirection(e.target.value as LayoutDirection)}
            title={t("layoutDirection")}
          >
            {DIRECTIONS.map((d) => (
              <option key={d.value} value={d.value}>{t(d.labelKey)}</option>
            ))}
          </select>
          <div className="h-4 w-px bg-[var(--border)]" />
          <button type="button" className="p-1 rounded hover:bg-[var(--surface)] text-[var(--text-muted)] hover:text-[var(--text)]" title={t("zoomIn")} onClick={handleZoomIn}>
            <ZoomIn className="h-3.5 w-3.5" />
          </button>
          <button type="button" className="p-1 rounded hover:bg-[var(--surface)] text-[var(--text-muted)] hover:text-[var(--text)]" title={t("zoomOut")} onClick={handleZoomOut}>
            <ZoomOut className="h-3.5 w-3.5" />
          </button>
          <button type="button" className="p-1 rounded hover:bg-[var(--surface)] text-[var(--text-muted)] hover:text-[var(--text)]" title={t("fitView")} onClick={handleFitView}>
            <Maximize2 className="h-3.5 w-3.5" />
          </button>
          <div className="h-4 w-px bg-[var(--border)]" />
          <button type="button" className="p-1 rounded hover:bg-[var(--surface)] text-[var(--text-muted)] hover:text-[var(--text)]" title={t("restoreWindow")} onClick={onClose}>
            <X className="h-4 w-4" />
          </button>
        </div>
        <div className="flex-1 min-h-0 relative">
          <EquationGraphView
            code={code}
            modelName={modelName}
            projectDir={projectDir}
            layoutOptions={layoutOptions}
            onReady={setPaperHandle}
          />
        </div>
      </div>
    </div>
  );
}
