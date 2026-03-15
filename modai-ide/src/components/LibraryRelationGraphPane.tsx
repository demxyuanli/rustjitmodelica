import { useCallback, useState } from "react";
import { ZoomIn, ZoomOut, Maximize2, Maximize } from "lucide-react";
import { t } from "../i18n";
import { EquationGraphView } from "./EquationGraphView";
import { DependencyGraphModal } from "./DependencyGraphModal";
import type { JointPaperHandle } from "../utils/jointUtils";

interface LibraryRelationGraphPaneProps {
  code: string | null;
  modelName: string | null;
  projectDir?: string | null;
}

export function LibraryRelationGraphPane({ code, modelName, projectDir }: LibraryRelationGraphPaneProps) {
  const [modalOpen, setModalOpen] = useState(false);
  const [paperHandle, setPaperHandle] = useState<JointPaperHandle | null>(null);
  const canShowGraph = Boolean(code && modelName);

  const handleZoomIn = useCallback(() => paperHandle?.zoomIn(), [paperHandle]);
  const handleZoomOut = useCallback(() => paperHandle?.zoomOut(), [paperHandle]);
  const handleFitView = useCallback(() => paperHandle?.fitView(), [paperHandle]);

  return (
    <div className="flex h-full min-h-[220px] flex-col">
      <div className="flex items-center justify-between border-b border-border px-3 py-1.5">
        <span className="text-xs font-medium text-[var(--text-muted)]">
          {t("libraryInternalRelationGraph")}
        </span>
        {canShowGraph && (
          <div className="flex items-center gap-0.5">
            <button type="button" className="p-1 rounded hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]" title={t("zoomIn")} onClick={handleZoomIn}>
              <ZoomIn className="h-3 w-3" />
            </button>
            <button type="button" className="p-1 rounded hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]" title={t("zoomOut")} onClick={handleZoomOut}>
              <ZoomOut className="h-3 w-3" />
            </button>
            <button type="button" className="p-1 rounded hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]" title={t("fitView")} onClick={handleFitView}>
              <Maximize2 className="h-3 w-3" />
            </button>
            <div className="w-px h-3 bg-[var(--border)] mx-0.5" />
            <button type="button" className="p-1 rounded hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--text)]" title={t("expandToWindow")} onClick={() => setModalOpen(true)}>
              <Maximize className="h-3 w-3" />
            </button>
          </div>
        )}
      </div>
      <div className="flex-1 min-h-0 relative overflow-hidden">
        {!canShowGraph ? (
          <div className="flex h-full items-center justify-center text-sm text-[var(--text-muted)]">
            {t("libraryNoSelection")}
          </div>
        ) : (
          <EquationGraphView
            code={code ?? ""}
            modelName={modelName ?? ""}
            projectDir={projectDir}
            layoutOptions={{ algorithm: "layered", direction: "RIGHT" }}
            onReady={setPaperHandle}
          />
        )}
      </div>
      {canShowGraph && (
        <DependencyGraphModal
          open={modalOpen}
          onClose={() => setModalOpen(false)}
          code={code ?? ""}
          modelName={modelName ?? ""}
          projectDir={projectDir}
        />
      )}
    </div>
  );
}
