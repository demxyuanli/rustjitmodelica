import ReactECharts from "echarts-for-react";
import { readCssPx, readCssVar } from "../settings/themes";

export function ResultsChart({
  time,
  series,
  onCursorIndex,
}: {
  time: number[];
  series: { name: string; data: number[]; dashed?: boolean }[];
  onCursorIndex: (idx: number | null) => void;
}) {
  const muted = readCssVar("--muted", "#9aa3ad");
  const text = readCssVar("--text", "#e8ecf1");
  const border = readCssVar("--border", "#3a424d");
  const fs = readCssPx("--studio-font-size", 13);

  const option = {
    backgroundColor: "transparent",
    animation: false,
    grid: { left: 48, right: 16, top: 28, bottom: 52 },
    tooltip: { trigger: "axis" },
    axisPointer: {
      type: "cross",
      label: { backgroundColor: border },
    },
    legend: { type: "scroll", top: 0, textStyle: { color: muted, fontSize: fs } },
    dataZoom: [
      { type: "inside", xAxisIndex: 0 },
      { type: "slider", height: 16, bottom: 8, xAxisIndex: 0 },
    ],
    xAxis: {
      type: "value",
      name: "t",
      axisLabel: { color: muted, fontSize: fs },
      axisLine: { lineStyle: { color: border } },
      nameTextStyle: { color: muted, fontSize: fs },
      splitLine: { show: false },
    },
    yAxis: {
      type: "value",
      axisLabel: { color: muted, fontSize: fs },
      splitLine: { lineStyle: { color: border } },
      axisLine: { lineStyle: { color: border } },
    },
    textStyle: { color: text, fontSize: fs },
    series: series.map((s) => ({
      name: s.name,
      type: "line",
      showSymbol: false,
      lineStyle: s.dashed ? { type: "dashed" } : undefined,
      data: time.map((t, i) => [t, s.data[i] ?? null]),
    })),
  };

  const onEvents = {
    updateAxisPointer: (ev: { axesInfo?: { value?: string | number }[] }) => {
      const raw = ev?.axesInfo?.[0]?.value;
      if (raw == null || !time.length) {
        onCursorIndex(null);
        return;
      }
      const tVal = Number(raw);
      let idx = 0;
      let best = Number.POSITIVE_INFINITY;
      for (let i = 0; i < time.length; i++) {
        const d = Math.abs(time[i]! - tVal);
        if (d < best) {
          best = d;
          idx = i;
        }
      }
      onCursorIndex(idx);
    },
  };

  return (
    <ReactECharts
      option={option}
      style={{ height: "100%", minHeight: 200 }}
      onEvents={onEvents}
    />
  );
}
