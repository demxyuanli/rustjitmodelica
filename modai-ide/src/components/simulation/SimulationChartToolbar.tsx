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
    <div className="flex shrink-0 items-center border-b border-border bg-surface-alt">
      {/* Left: view mode toggle — always visible */}
      <div className="shrink-0 px-2 py-1">
        <div className="flex items-center gap-1 rounded border border-border bg-surface p-0.5">
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
        <div className="min-w-0 flex-1 overflow-x-auto scroll-vscode px-1 py-1">
          <div className="flex min-w-max items-center gap-2">
            <button
              type="button"
              className="rounded border border-border px-2 py-1 text-xs theme-button-secondary"
              onClick={onExportCSV}
            >
              {t("exportCSV")}
            </button>
            <button
              type="button"
              className="rounded border border-border px-2 py-1 text-xs theme-button-secondary"
              onClick={onExportJSON}
            >
              {t("exportJSON")}
            </button>
          </div>
        </div>
      ) : (
        <div className="flex-1" />
      )}

      {/* Right: chart controls — always visible, pinned to the right */}
      <div className="flex shrink-0 items-center gap-2 border-l border-border px-2 py-1">
        <button
          type="button"
          className="rounded border border-border px-2 py-1 text-xs theme-button-secondary disabled:opacity-50"
          onClick={onResetChart}
          disabled={!canResetChart || simViewMode !== "chart"}
        >
          {t("reset")}
        </button>
        <button
          type="button"
          className="rounded border border-border px-2 py-1 text-xs theme-button-secondary disabled:opacity-50"
          onClick={onSaveImage}
          disabled={!canSaveImage || simViewMode !== "chart"}
        >
          {t("save")}
        </button>
        <button
          type="button"
          className="rounded border border-border px-2 py-1 text-xs theme-button-secondary disabled:opacity-50"
          onClick={onExpandChart}
          disabled={!canExpandChart || simViewMode !== "chart"}
        >
          {expandLabel}
        </button>
      </div>
    </div>
  );
}
