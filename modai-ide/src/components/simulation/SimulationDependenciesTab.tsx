import { useState, useEffect } from "react";
import { ZoomIn, ZoomOut, Maximize2, Maximize, Settings } from "lucide-react";
import { t } from "../../i18n";
import type { JointPaperHandle } from "../../utils/jointUtils";
import { EquationGraphView } from "../EquationGraphView";
import { DependencyGraphModal } from "../DependencyGraphModal";
import type { EquationGraphMode } from "../../api/tauri";
import type { DependencyGraphBehavior } from "../../utils/dependencyGraphBehavior";

export interface SimulationDependenciesTabProps {
  code: string;
  modelName: string;
  projectDir: string | null;
  dependencyGraphBehavior: DependencyGraphBehavior;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onFitView: () => void;
  dependencyGraphModalOpen: boolean;
  setDependencyGraphModalOpen: (open: boolean) => void;
  onDepPaperReady: (handle: JointPaperHandle | null) => void;
  onOpenDependencyGraphSettings?: () => void;
}

export function SimulationDependenciesTab({
  code,
  modelName,
  projectDir,
  dependencyGraphBehavior,
  onZoomIn,
  onZoomOut,
  onFitView,
  dependencyGraphModalOpen,
  setDependencyGraphModalOpen,
  onDepPaperReady,
  onOpenDependencyGraphSettings,
}: SimulationDependenciesTabProps) {
  const { initialGraphMode } = dependencyGraphBehavior;
  const [graphMode, setGraphMode] = useState<EquationGraphMode>(initialGraphMode);
  useEffect(() => {
    setGraphMode(initialGraphMode);
  }, [initialGraphMode]);

  const modeHelp =
    graphMode === "structural"
      ? t("dependencyGraphModeHelpStructural")
      : graphMode === "compact"
        ? t("dependencyGraphModeHelpCompact")
        : graphMode === "full"
          ? t("dependencyGraphModeHelpFull")
          : t("dependencyGraphModeHelpTopLevel");

  return (
    <div className="flex flex-1 min-h-0 flex-col">
      <div className="shrink-0 flex items-center justify-between px-2 py-1 border-b border-border">
        <span className="text-[10px] text-[var(--text-muted)]">
          {t("dependencyGraphTitle")}
        </span>
        <div className="flex items-center gap-1">
          <select
            className="rounded border border-[var(--border)] bg-[var(--bg-input)] text-[10px] text-[var(--text)] px-1 py-0.5"
            value={graphMode}
            onChange={(e) => setGraphMode(e.target.value as EquationGraphMode)}
            title={t("dependencyGraphMode")}
          >
            <option value="structural">{t("dependencyGraphModeStructural")}</option>
            <option value="compact">{t("dependencyGraphModeCompact")}</option>
            <option value="top-level">{t("dependencyGraphModeTopLevel")}</option>
            <option value="full">{t("dependencyGraphModeFull")}</option>
          </select>
          <span className="text-[10px] text-[var(--text-muted)]">{modeHelp}</span>
          <button
            type="button"
            className="p-1 rounded hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]"
            title={t("zoomIn")}
            onClick={onZoomIn}
          >
            <ZoomIn className="h-3 w-3" />
          </button>
          <button
            type="button"
            className="p-1 rounded hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]"
            title={t("zoomOut")}
            onClick={onZoomOut}
          >
            <ZoomOut className="h-3 w-3" />
          </button>
          <button
            type="button"
            className="p-1 rounded hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]"
            title={t("fitView")}
            onClick={onFitView}
          >
            <Maximize2 className="h-3 w-3" />
          </button>
          <div className="w-px h-3 bg-[var(--border)] mx-0.5" />
          <button
            type="button"
            className="p-1 rounded hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]"
            title={t("expandToWindow")}
            onClick={() => setDependencyGraphModalOpen(true)}
          >
            <Maximize className="h-3 w-3" />
          </button>
          {onOpenDependencyGraphSettings ? (
            <>
              <div className="w-px h-3 bg-[var(--border)] mx-0.5" />
              <button
                type="button"
                className="p-1 rounded hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]"
                title={t("dependencyGraphOpenSettings")}
                aria-label={t("dependencyGraphOpenSettings")}
                onClick={() => onOpenDependencyGraphSettings()}
              >
                <Settings className="h-3 w-3" />
              </button>
            </>
          ) : null}
        </div>
      </div>
      <div className="flex-1 min-h-0 relative overflow-hidden">
        <EquationGraphView
          code={code}
          modelName={modelName}
          projectDir={projectDir}
          graphMode={graphMode}
          onGraphModeChange={setGraphMode}
          dependencyGraphBehavior={dependencyGraphBehavior}
          layoutOptions={{ algorithm: "layered", direction: "RIGHT" }}
          onReady={onDepPaperReady}
        />
      </div>
      <DependencyGraphModal
        open={dependencyGraphModalOpen}
        onClose={() => setDependencyGraphModalOpen(false)}
        code={code}
        modelName={modelName}
        projectDir={projectDir}
        graphMode={graphMode}
        onGraphModeChange={setGraphMode}
        dependencyGraphBehavior={dependencyGraphBehavior}
        onOpenDependencyGraphSettings={onOpenDependencyGraphSettings}
      />
    </div>
  );
}
