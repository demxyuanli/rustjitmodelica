import { forwardRef, useImperativeHandle, useMemo, useRef } from "react";
import type { EChartsOption } from "echarts";
import EChartsReact from "echarts-for-react";
import { t } from "../../i18n";
import { useDiagramScheme } from "../../contexts/DiagramSchemeContext";
import type { SimulationChartMeta, SimulationChartSeries } from "./types";

export interface SimulationChartHandle {
  resetView: () => void;
  saveImage: () => void;
}

interface SimulationChartViewProps {
  theme: "dark" | "light";
  timeValues: number[];
  series: SimulationChartSeries[];
  meta: SimulationChartMeta;
  minHeight?: number;
  showSummary?: boolean;
}

function readThemeColor(name: string, fallback: string): string {
  if (typeof window === "undefined") return fallback;
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

function formatTooltipValue(value: unknown): string {
  if (typeof value !== "number" || Number.isNaN(value)) {
    return String(value ?? "");
  }
  const absValue = Math.abs(value);
  if ((absValue > 0 && absValue < 0.001) || absValue >= 10000) {
    return value.toExponential(4);
  }
  return value.toFixed(6).replace(/\.?0+$/, "");
}

export const SimulationChartView = forwardRef<SimulationChartHandle, SimulationChartViewProps>(
  function SimulationChartView({ theme, timeValues, series, meta, minHeight = 420, showSummary = true }, ref) {
    const chartRef = useRef<EChartsReact | null>(null);
    const { scheme } = useDiagramScheme();

    const chartOption = useMemo<EChartsOption>(() => {
      const paperBg = readThemeColor("--surface", theme === "light" ? "#f3f4f6" : "#1e1e1e");
      const plotBg = readThemeColor("--surface-elevated", theme === "light" ? "#ffffff" : "#202329");
      const textColor = readThemeColor("--text", theme === "light" ? "#111827" : "#d1d5db");
      const mutedText = readThemeColor("--text-muted", theme === "light" ? "#6b7280" : "#9ca3af");
      const borderColor = readThemeColor("--border", theme === "light" ? "#d1d5db" : "#353b45");
      const palette = theme === "light" ? scheme.chartPaletteLight : scheme.chartPaletteDark;

      return {
        animation: false,
        backgroundColor: plotBg,
        color: palette,
        grid: {
          left: 160,
          right: 24,
          top: 36,
          bottom: 64,
          containLabel: false,
        },
        legend: {
          type: "scroll",
          orient: "vertical",
          left: 8,
          top: "middle",
          width: 140,
          textStyle: {
            color: textColor,
            fontSize: 11,
          },
          pageTextStyle: {
            color: mutedText,
          },
          pageIconColor: mutedText,
          pageIconInactiveColor: borderColor,
        },
        toolbox: {
          top: 10,
          right: 10,
          iconStyle: {
            borderColor,
          },
          emphasis: {
            iconStyle: {
              borderColor: textColor,
            },
          },
          feature: {
            dataZoom: {
              yAxisIndex: "none",
            },
            restore: {},
            saveAsImage: {
              name: "simulation-chart",
              backgroundColor: paperBg,
            },
          },
        },
        tooltip: {
          trigger: "axis",
          axisPointer: {
            type: "cross",
            label: {
              backgroundColor: theme === "light" ? "#111827" : "#0f172a",
            },
          },
          valueFormatter: formatTooltipValue,
          backgroundColor: theme === "light" ? "rgba(255,255,255,0.96)" : "rgba(17,24,39,0.94)",
          borderColor,
          textStyle: {
            color: textColor,
          },
        },
        xAxis: {
          type: "value",
          name: t("time"),
          nameLocation: "middle",
          nameGap: 34,
          min: meta.xMin ?? undefined,
          max: meta.xMax ?? undefined,
          axisLine: {
            lineStyle: {
              color: borderColor,
            },
          },
          axisLabel: {
            color: mutedText,
          },
          splitLine: {
            lineStyle: {
              color: borderColor,
              opacity: 0.5,
            },
          },
        },
        yAxis: {
          type: "value",
          scale: true,
          axisLine: {
            lineStyle: {
              color: borderColor,
            },
          },
          axisLabel: {
            color: mutedText,
          },
          splitLine: {
            lineStyle: {
              color: borderColor,
              opacity: 0.5,
            },
          },
        },
        dataZoom: [
          {
            type: "inside",
            xAxisIndex: 0,
            filterMode: "none",
            zoomOnMouseWheel: true,
            moveOnMouseMove: true,
          },
          {
            type: "slider",
            xAxisIndex: 0,
            filterMode: "none",
            bottom: 20,
            height: 18,
            borderColor,
            backgroundColor: paperBg,
            fillerColor: theme === "light" ? "rgba(37,99,235,0.16)" : "rgba(96,165,250,0.16)",
            handleStyle: {
              color: palette[0],
            },
            textStyle: {
              color: mutedText,
            },
          },
        ],
        series: series.map((item) => ({
          name: item.name,
          type: "line",
          data: timeValues.map((timeValue, index) => [timeValue, item.values[index]]),
          showSymbol: false,
          sampling: "lttb",
          smooth: false,
          emphasis: {
            focus: "series",
          },
          lineStyle: {
            width: 1.8,
          },
        })),
      };
    }, [meta.xMax, meta.xMin, scheme, series, theme, timeValues]);

    useImperativeHandle(ref, () => ({
      resetView() {
        chartRef.current?.getEchartsInstance().dispatchAction({ type: "restore" });
      },
      saveImage() {
        const instance = chartRef.current?.getEchartsInstance();
        if (!instance) return;
        const link = document.createElement("a");
        link.href = instance.getDataURL({
          type: "png",
          pixelRatio: 2,
          backgroundColor: readThemeColor("--surface", theme === "light" ? "#f3f4f6" : "#1e1e1e"),
        });
        link.download = "simulation-chart.png";
        link.click();
      },
    }), [theme]);

    const chartMinHeight = `${minHeight}px`;

    if (series.length === 0 || timeValues.length === 0) {
      return (
        <div className="flex h-full min-h-[220px] items-center justify-center text-sm text-[var(--text-muted)]" style={{ minHeight: chartMinHeight }}>
          {t("runSimulationToSeePlot")}
        </div>
      );
    }

    return (
      <div className="flex h-full min-h-0 flex-col" style={{ minHeight: chartMinHeight }}>
        {showSummary && (
          <div className="flex items-center gap-2 border-b border-border bg-surface px-3 py-2 text-[11px] text-[var(--text-muted)]">
            <span>{meta.seriesCount} {t("chartSeries")}</span>
            <span>{meta.pointCount} {t("chartSamples")}</span>
            {meta.xMin != null && meta.xMax != null && (
              <span>
                {t("chartRange")}: {formatTooltipValue(meta.xMin)} - {formatTooltipValue(meta.xMax)}
              </span>
            )}
          </div>
        )}
        <div className="min-h-0 flex-1 bg-[var(--surface)]">
          <EChartsReact
            ref={chartRef}
            option={chartOption}
            notMerge
            lazyUpdate
            style={{ width: "100%", height: "100%" }}
          />
        </div>
      </div>
    );
  }
);
