import { useMemo, useState } from "react";
import ReactECharts from "echarts-for-react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  Box,
  ChartScatter,
  FolderOpen,
  History,
  LineChart,
  MessageSquare,
  Package,
  Table2,
  Timer,
} from "lucide-react";
import type {
  DiagramDocument,
  EmitRecord,
  RunRecord,
  SimProfile,
  SimulationResult,
  StudioReport,
} from "../api/types";
import { resolveSeriesKeys } from "../app/plotHints";
import { t, type MessageKey } from "../i18n";
import { MultibodyViewport } from "../multibody/MultibodyViewport";
import { readCssPx, readCssVar } from "../settings/themes";
import { IconButton } from "../shell/IconButton";
import { DataTable } from "./DataTable";
import { MessagesPanel } from "./MessagesPanel";
import { ProfilerPanel } from "./ProfilerPanel";
import { ResultsChart } from "./ResultsChart";
import { RunsPanel } from "./RunsPanel";
import { displaySeries, instanceNamesFromNodes } from "./seriesDisplay";
import { VarTree } from "./VarTree";

export type ResultsDockTab =
  | "results"
  | "data"
  | "messages"
  | "runs"
  | "xy"
  | "3d"
  | "profiler"
  | "export";

export function ResultsDock({
  tab,
  onTab,
  result,
  selectedVars,
  onToggleVar,
  report,
  busy,
  progressText,
  onMessageClick,
  runs,
  overlayRunIds,
  onToggleOverlayRun,
  xyX,
  xyY,
  onXyX,
  onXyY,
  diagram,
  profile,
  profileWallMs,
  profileRunId,
  onSelectProfileRun,
  emitHistory,
}: {
  tab: ResultsDockTab;
  onTab: (t: ResultsDockTab) => void;
  result: SimulationResult | null;
  selectedVars: string[];
  onToggleVar: (name: string) => void;
  report: StudioReport | null;
  busy: boolean;
  progressText: string | null;
  onMessageClick: (d: { message: string; nodeId?: string }) => void;
  runs: RunRecord[];
  overlayRunIds: string[];
  onToggleOverlayRun: (runId: string) => void;
  xyX: string;
  xyY: string;
  onXyX: (v: string) => void;
  onXyY: (v: string) => void;
  diagram: DiagramDocument | null;
  profile: SimProfile | null;
  profileWallMs?: number | null;
  profileRunId: string | null;
  onSelectProfileRun: (runId: string) => void;
  emitHistory: EmitRecord[];
}) {
  const [cursorIndex, setCursorIndex] = useState<number | null>(null);
  const keys = result ? Object.keys(result.series).sort() : [];
  const overlayRuns = runs.filter((r) => overlayRunIds.includes(r.runId));
  const plottedVars =
    result && selectedVars.length ? resolveSeriesKeys(keys, selectedVars) : selectedVars;
  const nodes = diagram?.nodes ?? [];
  const names = instanceNamesFromNodes(nodes);
  const labels: Record<string, string> = {};
  for (const k of keys) labels[k] = displaySeries(k, names).label;

  const chartSeries: { name: string; data: number[]; dashed?: boolean }[] = plottedVars
    .filter((k) => result?.series[k])
    .map((k) => ({
      name: labels[k] ?? k,
      data: result!.series[k]!,
    }));
  for (const run of overlayRuns) {
    for (const k of plottedVars) {
      if (!run.result.series[k]) continue;
      chartSeries.push({
        name: `${run.label}:${labels[k] ?? k}`,
        data: run.result.series[k]!,
        dashed: true,
      });
    }
  }

  const time = result?.time ?? overlayRuns[0]?.result.time ?? [];
  const muted = readCssVar("--muted", "#9aa3ad");
  const text = readCssVar("--text", "#e8ecf1");
  const border = readCssVar("--border", "#3a424d");
  const fs = readCssPx("--studio-font-size", 13);

  const cursorReadout = useMemo(() => {
    if (cursorIndex == null || !result || !time[cursorIndex]) return null;
    const parts = plottedVars
      .filter((k) => result.series[k]?.[cursorIndex] != null)
      .slice(0, 4)
      .map((k) => `${labels[k] ?? k}=${Number(result.series[k]![cursorIndex]).toPrecision(4)}`);
    return [`t=${time[cursorIndex]}`, ...parts].join("  ");
  }, [cursorIndex, labels, plottedVars, result, time]);

  const xs = result?.series[xyX] ?? [];
  const ys = result?.series[xyY] ?? [];
  const n = Math.min(xs.length, ys.length);
  const xyData: [number, number][] = [];
  for (let i = 0; i < n; i++) xyData.push([xs[i]!, ys[i]!]);

  const xyOption = {
    backgroundColor: "transparent",
    animation: false,
    grid: { left: 48, right: 16, top: 24, bottom: 36 },
    tooltip: { trigger: "item" },
    xAxis: {
      type: "value",
      name: xyX || "x",
      axisLabel: { color: muted, fontSize: fs },
      splitLine: { lineStyle: { color: border } },
      nameTextStyle: { color: muted, fontSize: fs },
    },
    yAxis: {
      type: "value",
      name: xyY || "y",
      axisLabel: { color: muted, fontSize: fs },
      splitLine: { lineStyle: { color: border } },
      nameTextStyle: { color: muted, fontSize: fs },
    },
    textStyle: { color: text, fontSize: fs },
    series: [{ type: "scatter", symbolSize: 4, data: xyData }],
  };

  const openFolder = (path: string) => {
    void revealItemInDir(path).catch(() => {
      /* opener may be unavailable in browser-only vite */
    });
  };

  const tabBtn = (id: ResultsDockTab, icon: typeof LineChart, labelKey: MessageKey, extra?: string) => (
    <IconButton
      icon={icon}
      labelKey={labelKey}
      active={tab === id}
      title={extra != null ? `${t(labelKey)} (${extra})` : undefined}
      onClick={() => onTab(id)}
    />
  );

  return (
    <div className="bottom-dock">
      <div className="tabs">
        {tabBtn("results", LineChart, "results")}
        {tabBtn("data", Table2, "tabData")}
        {tabBtn("xy", ChartScatter, "xy")}
        {tabBtn("3d", Box, "tab3d")}
        {tabBtn("messages", MessageSquare, "messages")}
        {tabBtn("runs", History, "runs", String(runs.length))}
        {tabBtn("profiler", Timer, "profiler")}
        {tabBtn("export", Package, "exportTab", String(emitHistory.length))}
        {busy ? <span className="status-pill">{progressText ?? t("running")}</span> : null}
      </div>
      <div className={`tab-body${tab === "results" || tab === "data" ? " tab-body-fill" : ""}`}>
        {tab === "results" ? (
          <div className="results-split">
            <VarTree
              keys={keys}
              selectedVars={selectedVars}
              pendingVars={selectedVars}
              nodes={nodes}
              onToggle={onToggleVar}
            />
            <div className="results-plot">
              {keys.length && plottedVars.length ? (
                <>
                  <ResultsChart
                    time={time}
                    series={chartSeries}
                    onCursorIndex={setCursorIndex}
                  />
                  {cursorReadout ? <div className="cursor-readout">{cursorReadout}</div> : null}
                </>
              ) : (
                <div style={{ color: "var(--muted)" }}>{t("resultsNoSeries")}</div>
              )}
            </div>
          </div>
        ) : null}
        {tab === "data" ? (
          <DataTable
            time={time}
            columns={plottedVars.filter((k) => result?.series[k])}
            labels={labels}
            series={result?.series ?? {}}
            cursorIndex={cursorIndex}
          />
        ) : null}
        {tab === "xy" ? (
          <>
            <div className="var-list">
              <label>
                X{" "}
                <select value={xyX} onChange={(ev) => onXyX(ev.target.value)}>
                  <option value="">(pick)</option>
                  {keys.map((k) => (
                    <option key={k} value={k}>
                      {labels[k] ?? k}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Y{" "}
                <select value={xyY} onChange={(ev) => onXyY(ev.target.value)}>
                  <option value="">(pick)</option>
                  {keys.map((k) => (
                    <option key={k} value={k}>
                      {labels[k] ?? k}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            {xyX && xyY && xyData.length ? (
              <ReactECharts option={xyOption} style={{ height: "100%", minHeight: 200 }} />
            ) : (
              <div style={{ color: "var(--muted)" }}>{t("resultsPickXy")}</div>
            )}
          </>
        ) : null}
        {tab === "3d" ? <MultibodyViewport diagram={diagram} result={result} /> : null}
        {tab === "messages" ? (
          <MessagesPanel report={report} onMessageClick={onMessageClick} />
        ) : null}
        {tab === "runs" ? (
          <RunsPanel
            runs={runs}
            overlayRunIds={overlayRunIds}
            onToggleOverlay={onToggleOverlayRun}
            onOpenProfiler={(id) => {
              onSelectProfileRun(id);
              onTab("profiler");
            }}
          />
        ) : null}
        {tab === "profiler" ? (
          <ProfilerPanel
            profile={profile}
            wallMs={profileWallMs}
            runs={runs}
            selectedRunId={profileRunId}
            onSelectRun={onSelectProfileRun}
          />
        ) : null}
        {tab === "export" ? (
          <div>
            {!emitHistory.length ? (
              <div style={{ color: "var(--muted)" }}>{t("exportHistoryEmpty")}</div>
            ) : (
              emitHistory.map((rec) => (
                <div key={rec.id} className="export-history-item">
                  <span className={rec.success ? "ok" : "error"}>
                    {rec.success ? "OK" : "FAIL"}
                  </span>
                  <span>
                    {rec.mode} / {rec.packageKind}
                  </span>
                  <span style={{ color: "var(--muted)" }}>
                    {rec.wallMs != null ? `${rec.wallMs}ms` : ""} · {rec.files.length} files
                  </span>
                  <IconButton icon={FolderOpen} labelKey="exportOpenFolder" onClick={() => openFolder(rec.outDir)} />
                  {rec.fmuPath ? (
                    <IconButton
                      icon={Package}
                      labelKey="exportOpenFmu"
                      onClick={() => openFolder(rec.fmuPath!)}
                    />
                  ) : null}
                </div>
              ))
            )}
          </div>
        ) : null}
      </div>
    </div>
  );
}
