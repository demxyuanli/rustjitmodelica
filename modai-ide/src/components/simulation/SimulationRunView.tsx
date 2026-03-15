import { useRef, useState } from "react";
import { RotateCcw, Image, X } from "lucide-react";
import type { SimulationResult } from "../../types";
import { t } from "../../i18n";
import type { SimulationChartMeta, SimulationChartSeries } from "./types";
import { AppIcon } from "../Icon";
import { IconButton } from "../IconButton";
import { SimulationChartToolbar } from "./SimulationChartToolbar";
import { SimulationChartView, type SimulationChartHandle } from "./SimulationChartView";
import { SimulationTableView } from "./SimulationTableView";
import { SimulationVariablePicker } from "./SimulationVariablePicker";

interface SimulationRunViewProps {
  theme: "dark" | "light";
  simResult: SimulationResult | null;
  timeValues: number[];
  plotSeries: SimulationChartSeries[];
  chartMeta: SimulationChartMeta;
  allPlotVarNames: string[];
  selectedPlotVars: string[];
  tableSortKey: string;
  tableSortAsc: boolean;
  tablePage: number;
  tablePageSize: number;
  visibleTableColumns: string[];
  tableColumns: string[];
  sortedTableRows: Record<string, number>[];
  simViewMode: "chart" | "table";
  onViewModeChange: (mode: "chart" | "table") => void;
  onSortKeyChange: (value: string) => void;
  onSortAscChange: (value: boolean) => void;
  onPageChange: (value: number) => void;
  onPageSizeChange: (value: number) => void;
  onVisibleColumnsChange: (value: string[]) => void;
  onSelectPlotVars: (value: string[]) => void;
  onTogglePlotVar: (name: string) => void;
  onExportCSV: () => void;
  onExportJSON: () => void;
}

export function SimulationRunView({
  theme,
  simResult,
  timeValues,
  plotSeries,
  chartMeta,
  allPlotVarNames,
  selectedPlotVars,
  tableSortKey,
  tableSortAsc,
  tablePage,
  tablePageSize,
  visibleTableColumns,
  tableColumns,
  sortedTableRows,
  simViewMode,
  onViewModeChange,
  onSortKeyChange,
  onSortAscChange,
  onPageChange,
  onPageSizeChange,
  onVisibleColumnsChange,
  onSelectPlotVars,
  onTogglePlotVar,
  onExportCSV,
  onExportJSON,
}: SimulationRunViewProps) {
  const chartRef = useRef<SimulationChartHandle | null>(null);
  const modalChartRef = useRef<SimulationChartHandle | null>(null);
  const [isExpanded, setIsExpanded] = useState(false);
  const [modalViewMode, setModalViewMode] = useState<"chart" | "table">("chart");

  return (
    <>
      <div className="flex min-h-0 min-w-0 flex-1 overflow-hidden">
        <SimulationVariablePicker
          variableNames={allPlotVarNames}
          selectedNames={selectedPlotVars}
          onToggleVariable={onTogglePlotVar}
          onSelectAll={() => onSelectPlotVars([...allPlotVarNames])}
          onClearAll={() => onSelectPlotVars([])}
        />
        <div className="flex min-h-0 min-w-0 w-0 flex-1 flex-col overflow-hidden">
        <SimulationChartToolbar
          simViewMode={simViewMode}
          onViewModeChange={onViewModeChange}
          canExport={Boolean(simResult)}
          canResetChart={plotSeries.length > 0}
          canSaveImage={plotSeries.length > 0}
          canExpandChart={plotSeries.length > 0}
          expandLabel={t("maximize")}
          onExportCSV={onExportCSV}
          onExportJSON={onExportJSON}
          onResetChart={() => chartRef.current?.resetView()}
          onSaveImage={() => chartRef.current?.saveImage()}
          onExpandChart={() => {
            setModalViewMode(simViewMode);
            setIsExpanded(true);
          }}
        />

          <div className={`min-h-0 min-w-0 flex flex-1 flex-col ${simViewMode === "chart" ? "overflow-auto scroll-vscode" : "overflow-hidden"}`}>
          {simViewMode === "chart" ? (
            <SimulationChartView
              ref={chartRef}
              theme={theme}
              timeValues={timeValues}
              series={plotSeries}
              meta={chartMeta}
              minHeight={420}
            />
          ) : (
            <SimulationTableView
              tableSortKey={tableSortKey}
              tableSortAsc={tableSortAsc}
              tablePage={tablePage}
              tablePageSize={tablePageSize}
              visibleTableColumns={visibleTableColumns}
              tableColumns={tableColumns}
              sortedTableRows={sortedTableRows}
              onSortKeyChange={onSortKeyChange}
              onSortAscChange={onSortAscChange}
              onPageChange={onPageChange}
              onPageSizeChange={onPageSizeChange}
              onVisibleColumnsChange={onVisibleColumnsChange}
            />
          )}
          </div>
        </div>
      </div>

      {isExpanded && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6">
          <div className="flex h-full max-h-[92vh] w-full max-w-[1600px] min-w-0 flex-col overflow-hidden rounded-lg border border-border bg-surface-alt shadow-2xl">
            <div className="flex items-center gap-2 border-b border-border bg-surface px-3 py-2">
              <div className="text-sm font-medium text-[var(--text)]">{t("chartView")}</div>
              <div className="ml-3 flex items-center gap-1 rounded border border-border bg-surface-alt p-0.5">
                <IconButton
                  icon={<AppIcon name="chart" aria-hidden="true" />}
                  size="xs"
                  active={modalViewMode === "chart"}
                  onClick={() => setModalViewMode("chart")}
                  title={t("chartView")}
                  aria-label={t("chartView")}
                />
                <IconButton
                  icon={<AppIcon name="table" aria-hidden="true" />}
                  size="xs"
                  active={modalViewMode === "table"}
                  onClick={() => setModalViewMode("table")}
                  title={t("tableView")}
                  aria-label={t("tableView")}
                />
              </div>
              <div className="ml-auto flex items-center gap-1">
                <button
                  type="button"
                  className="rounded border border-border p-1.5 theme-button-secondary disabled:opacity-50"
                  onClick={() => modalChartRef.current?.resetView()}
                  disabled={plotSeries.length === 0 || modalViewMode !== "chart"}
                  title={t("reset")}
                >
                  <RotateCcw className="h-4 w-4" />
                </button>
                <button
                  type="button"
                  className="rounded border border-border p-1.5 theme-button-secondary disabled:opacity-50"
                  onClick={() => modalChartRef.current?.saveImage()}
                  disabled={plotSeries.length === 0 || modalViewMode !== "chart"}
                  title={t("save")}
                >
                  <Image className="h-4 w-4" />
                </button>
                <button
                  type="button"
                  className="rounded border border-border p-1.5 theme-button-secondary"
                  onClick={() => setIsExpanded(false)}
                  title={t("closeTab")}
                >
                  <X className="h-4 w-4" />
                </button>
              </div>
            </div>

            <div className="min-h-0 flex-1 overflow-hidden bg-[var(--surface)]">
              {modalViewMode === "chart" ? (
                <div className="h-full overflow-auto scroll-vscode p-3">
                  <SimulationChartView
                    ref={modalChartRef}
                    theme={theme}
                    timeValues={timeValues}
                    series={plotSeries}
                    meta={chartMeta}
                    minHeight={720}
                  />
                </div>
              ) : (
                <SimulationTableView
                  tableSortKey={tableSortKey}
                  tableSortAsc={tableSortAsc}
                  tablePage={tablePage}
                  tablePageSize={tablePageSize}
                  visibleTableColumns={visibleTableColumns}
                  tableColumns={tableColumns}
                  sortedTableRows={sortedTableRows}
                  onSortKeyChange={onSortKeyChange}
                  onSortAscChange={onSortAscChange}
                  onPageChange={onPageChange}
                  onPageSizeChange={onPageSizeChange}
                  onVisibleColumnsChange={onVisibleColumnsChange}
                />
              )}
            </div>
          </div>
        </div>
      )}
    </>
  );
}
