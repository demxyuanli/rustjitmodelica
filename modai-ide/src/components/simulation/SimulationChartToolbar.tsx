import { FileDown, FileJson, RotateCcw, Image, Maximize2 } from "lucide-react";
import { t } from "../../i18n";
import { AppIcon } from "../Icon";
import { IconButton } from "../IconButton";

interface SimulationChartToolbarProps {
  simViewMode: "chart" | "table";
  onViewModeChange: (mode: "chart" | "table") => void;
  canExport: boolean;
  canResetChart: boolean;
  canSaveImage: boolean;
  canExpandChart: boolean;
  expandLabel: string;
  onExportCSV: () => void;
  onExportJSON: () => void;
  onResetChart: () => void;
  onSaveImage: () => void;
  onExpandChart: () => void;
}

export function SimulationChartToolbar({
  simViewMode,
  onViewModeChange,
  canExport,
  canResetChart,
  canSaveImage,
  canExpandChart,
  expandLabel,
  onExportCSV,
  onExportJSON,
  onResetChart,
  onSaveImage,
  onExpandChart,
}: SimulationChartToolbarProps) {
  return (
    <div className="panel-header-min-height flex shrink-0 items-center border-b border-border bg-surface-alt">
      {/* Left: view mode toggle — always visible */}
      <div className="panel-header-padding shrink-0">
        <div className="flex items-center gap-[var(--toolbar-gap)] rounded border border-border bg-surface p-0.5">
          <IconButton
            icon={<AppIcon name="chart" aria-hidden="true" />}
            size="xs"
            active={simViewMode === "chart"}
            onClick={() => onViewModeChange("chart")}
            title={t("chartView")}
            aria-label={t("chartView")}
          />
          <IconButton
            icon={<AppIcon name="table" aria-hidden="true" />}
            size="xs"
            active={simViewMode === "table"}
            onClick={() => onViewModeChange("table")}
            title={t("tableView")}
            aria-label={t("tableView")}
          />
        </div>
      </div>

      {/* Middle: export buttons (scrollable when space is limited) */}
      {canExport ? (
        <div className="min-w-0 flex-1 overflow-x-auto scroll-vscode panel-header-padding">
          <div className="flex min-w-max items-center gap-[var(--toolbar-gap)]">
            <button
              type="button"
              className="toolbar-icon-btn flex rounded items-center justify-center border border-border theme-button-secondary"
              onClick={onExportCSV}
              title={t("exportCSV")}
            >
              <FileDown className="h-4 w-4" />
            </button>
            <button
              type="button"
              className="toolbar-icon-btn flex rounded items-center justify-center border border-border theme-button-secondary"
              onClick={onExportJSON}
              title={t("exportJSON")}
            >
              <FileJson className="h-4 w-4" />
            </button>
          </div>
        </div>
      ) : (
        <div className="flex-1" />
      )}

      {/* Right: chart controls — always visible, pinned to the right */}
      <div className="panel-header-padding flex shrink-0 items-center gap-[var(--toolbar-gap)] border-l border-border">
        <button
          type="button"
          className="toolbar-icon-btn flex rounded items-center justify-center border border-border theme-button-secondary disabled:opacity-50"
          onClick={onResetChart}
          disabled={!canResetChart || simViewMode !== "chart"}
          title={t("reset")}
        >
          <RotateCcw className="h-4 w-4" />
        </button>
        <button
          type="button"
          className="toolbar-icon-btn flex rounded items-center justify-center border border-border theme-button-secondary disabled:opacity-50"
          onClick={onSaveImage}
          disabled={!canSaveImage || simViewMode !== "chart"}
          title={t("save")}
        >
          <Image className="h-4 w-4" />
        </button>
        <button
          type="button"
          className="toolbar-icon-btn flex rounded items-center justify-center border border-border theme-button-secondary disabled:opacity-50"
          onClick={onExpandChart}
          disabled={!canExpandChart || simViewMode !== "chart"}
          title={expandLabel}
        >
          <Maximize2 className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
}
