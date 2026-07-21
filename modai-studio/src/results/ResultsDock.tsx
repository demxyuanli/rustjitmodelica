import ReactECharts from "echarts-for-react";
import type { RunRecord, SimulationResult, StudioReport } from "../api/types";

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
}: {
  tab: "results" | "messages" | "runs" | "xy";
  onTab: (t: "results" | "messages" | "runs" | "xy") => void;
  result: SimulationResult | null;
  selectedVars: string[];
  onToggleVar: (name: string) => void;
  report: StudioReport | null;
  busy: boolean;
  progressText: string | null;
  onMessageClick: (message: string) => void;
  runs: RunRecord[];
  overlayRunIds: string[];
  onToggleOverlayRun: (runId: string) => void;
  xyX: string;
  xyY: string;
  onXyX: (v: string) => void;
  onXyY: (v: string) => void;
}) {
  const keys = result ? Object.keys(result.series).sort() : [];
  const overlayRuns = runs.filter((r) => overlayRunIds.includes(r.runId));

  const series: object[] = selectedVars
    .filter((k) => result?.series[k])
    .map((k) => ({
      name: k,
      type: "line",
      showSymbol: false,
      data: result!.series[k],
    }));

  for (const run of overlayRuns) {
    for (const k of selectedVars) {
      if (!run.result.series[k]) continue;
      series.push({
        name: `${run.label}:${k}`,
        type: "line",
        showSymbol: false,
        lineStyle: { type: "dashed" },
        data: run.result.series[k],
      });
    }
  }

  const option = {
    backgroundColor: "transparent",
    animation: false,
    grid: { left: 40, right: 16, top: 24, bottom: 28 },
    tooltip: { trigger: "axis" },
    legend: { type: "scroll", top: 0, textStyle: { color: "#9aa3ad", fontSize: 10 } },
    xAxis: { type: "category", data: result?.time ?? overlayRuns[0]?.result.time ?? [], name: "t" },
    yAxis: { type: "value" },
    series,
  };

  const xs = result?.series[xyX] ?? [];
  const ys = result?.series[xyY] ?? [];
  const n = Math.min(xs.length, ys.length);
  const xyData: [number, number][] = [];
  for (let i = 0; i < n; i++) xyData.push([xs[i], ys[i]]);

  const xyOption = {
    backgroundColor: "transparent",
    animation: false,
    grid: { left: 48, right: 16, top: 24, bottom: 36 },
    tooltip: { trigger: "item" },
    xAxis: { type: "value", name: xyX || "x" },
    yAxis: { type: "value", name: xyY || "y" },
    series: [
      {
        type: "scatter",
        symbolSize: 4,
        data: xyData,
      },
    ],
  };

  return (
    <div className="bottom-dock">
      <div className="tabs">
        <button type="button" className={tab === "results" ? "active" : ""} onClick={() => onTab("results")}>
          Results
        </button>
        <button type="button" className={tab === "xy" ? "active" : ""} onClick={() => onTab("xy")}>
          XY
        </button>
        <button type="button" className={tab === "messages" ? "active" : ""} onClick={() => onTab("messages")}>
          Messages
        </button>
        <button type="button" className={tab === "runs" ? "active" : ""} onClick={() => onTab("runs")}>
          Runs ({runs.length})
        </button>
        {busy ? <span className="status-pill">{progressText ?? "Running..."}</span> : null}
      </div>
      <div className="tab-body">
        {tab === "results" ? (
          <>
            <div className="var-list">
              {keys.map((k) => (
                <label key={k}>
                  <input
                    type="checkbox"
                    checked={selectedVars.includes(k)}
                    onChange={() => onToggleVar(k)}
                  />{" "}
                  {k}
                </label>
              ))}
              {!keys.length ? <span style={{ color: "var(--muted)" }}>No series yet. Click Run.</span> : null}
            </div>
            {selectedVars.length ? (
              <ReactECharts option={option} style={{ height: 180 }} />
            ) : null}
          </>
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
                      {k}
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
                      {k}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            {xyX && xyY && xyData.length ? (
              <ReactECharts option={xyOption} style={{ height: 180 }} />
            ) : (
              <div style={{ color: "var(--muted)" }}>Pick two series for phase plane</div>
            )}
          </>
        ) : null}
        {tab === "messages" ? (
          <div>
            {report ? (
              <>
                <div className={`msg-item ${report.success ? "ok" : "error"}`}>
                  [{report.stage}] {report.success ? "OK" : "Failed"}
                </div>
                {report.errors.map((e, i) => (
                  <button
                    type="button"
                    className="msg-item error msg-click"
                    key={i}
                    onClick={() => onMessageClick(e.message)}
                  >
                    {e.message}
                  </button>
                ))}
                {report.warnings.map((w, i) => (
                  <button
                    type="button"
                    className="msg-item msg-click"
                    key={`w${i}`}
                    onClick={() => onMessageClick(w.message)}
                  >
                    {w.message}
                  </button>
                ))}
              </>
            ) : (
              <div style={{ color: "var(--muted)" }}>No messages</div>
            )}
          </div>
        ) : null}
        {tab === "runs" ? (
          <div>
            {!runs.length ? (
              <div style={{ color: "var(--muted)" }}>No runs yet. Each successful Run is stored here.</div>
            ) : (
              <>
                <div style={{ fontSize: 12, color: "var(--muted)", marginBottom: 6 }}>
                  Check runs to overlay on Results (dashed). wallMs = profiler wall time.
                </div>
                {runs.map((r) => (
                  <label key={r.runId} className="msg-item" style={{ display: "flex", gap: 8, alignItems: "center" }}>
                    <input
                      type="checkbox"
                      checked={overlayRunIds.includes(r.runId)}
                      onChange={() => onToggleOverlayRun(r.runId)}
                    />
                    <span>
                      {r.label} — {r.createdAt}
                      {r.wallMs != null ? ` — ${r.wallMs}ms` : ""} — solver={r.options.solver}
                    </span>
                  </label>
                ))}
                {overlayRunIds.length && selectedVars.length ? (
                  <ReactECharts option={option} style={{ height: 160, marginTop: 8 }} />
                ) : null}
              </>
            )}
          </div>
        ) : null}
      </div>
    </div>
  );
}
