import ReactECharts from "echarts-for-react";
import type { RunRecord, SimProfile } from "../api/types";
import { t } from "../i18n";
import { readCssPx, readCssVar } from "../settings/themes";

type PhaseRow = { key: string; label: string; ms: number; skipped?: boolean };

function phaseRows(p: SimProfile): PhaseRow[] {
  return [
    { key: "emit", label: t("profilePhaseEmit"), ms: p.emitMs },
    {
      key: "compile",
      label: t("profilePhaseCompile"),
      ms: p.compileMs,
      skipped: p.compileSkipped,
    },
    { key: "apply", label: t("profilePhaseApply"), ms: p.applyParamsMs },
    { key: "integrate", label: t("profilePhaseIntegrate"), ms: p.integrateMs },
  ];
}

function modeLabel(mode?: string | null): string {
  if (mode === "fastRestart") return t("modeFastRestart");
  if (mode === "fullCompile") return t("modeFullCompile");
  if (mode === "fmu") return "FMU";
  return t("modeRun");
}

export function ProfilerPanel({
  profile,
  wallMs,
  runs,
  selectedRunId,
  onSelectRun,
}: {
  profile: SimProfile | null;
  wallMs?: number | null;
  runs: RunRecord[];
  selectedRunId: string | null;
  onSelectRun: (runId: string) => void;
}) {
  const muted = readCssVar("--muted", "#9aa3ad");
  const text = readCssVar("--text", "#e8ecf1");
  const accent = readCssVar("--accent", "#3d8bfd");
  const border = readCssVar("--border", "#3a424d");
  const fs = readCssPx("--studio-font-size", 13);

  if (!profile) {
    return (
      <div className="profiler-empty" style={{ color: muted, padding: 8 }}>
        {t("profilerEmpty")}
      </div>
    );
  }

  const rows = phaseRows(profile);
  const total = Math.max(profile.totalMs, 1);
  const option = {
    backgroundColor: "transparent",
    animation: false,
    grid: { left: 88, right: 48, top: 12, bottom: 12 },
    xAxis: {
      type: "value",
      axisLabel: { color: muted, fontSize: fs },
      splitLine: { lineStyle: { color: border } },
      name: "ms",
      nameTextStyle: { color: muted, fontSize: fs },
    },
    yAxis: {
      type: "category",
      data: rows.map((r) => r.label),
      axisLabel: { color: text, fontSize: fs },
      axisLine: { lineStyle: { color: border } },
    },
    textStyle: { color: text },
    series: [
      {
        type: "bar",
        data: rows.map((r) => ({
          value: r.ms,
          itemStyle: {
            color: r.skipped ? muted : accent,
            opacity: r.skipped ? 0.45 : 1,
          },
        })),
        label: {
          show: true,
          position: "right",
          color: muted,
          fontSize: fs,
          formatter: (p: { value: number }) => `${p.value} ms`,
        },
        barMaxWidth: 18,
      },
    ],
  };

  return (
    <div className="profiler-panel">
      <div className="profiler-meta">
        <span>
          {t("profilerTotal")}: <strong>{profile.totalMs}</strong> ms
          {wallMs != null ? (
            <span style={{ color: muted }}> ({t("profilerClientWall")}: {wallMs} ms)</span>
          ) : null}
        </span>
        <span>
          {t("profilerMode")}: {modeLabel(profile.restartMode)}
          {profile.compileSkipped ? ` · ${t("profilerCompileSkipped")}` : ""}
        </span>
      </div>
      <div className="profiler-body">
        <table className="profiler-table">
          <thead>
            <tr>
              <th>{t("profilerPhase")}</th>
              <th>{t("profilerMs")}</th>
              <th>{t("profilerPct")}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.key} className={r.skipped ? "skipped" : undefined}>
                <td>
                  {r.label}
                  {r.skipped ? ` (${t("profilerSkipped")})` : ""}
                </td>
                <td>{r.ms}</td>
                <td>{((r.ms / total) * 100).toFixed(1)}%</td>
              </tr>
            ))}
          </tbody>
        </table>
        <ReactECharts option={option} style={{ height: 160, flex: 1, minWidth: 220 }} />
      </div>
      {runs.length > 1 ? (
        <div className="profiler-runs">
          <span style={{ color: muted }}>{t("profilerCompareRuns")}</span>
          <select
            value={selectedRunId ?? ""}
            onChange={(ev) => onSelectRun(ev.target.value)}
          >
            {runs
              .filter((r) => r.profile)
              .map((r) => (
                <option key={r.runId} value={r.runId}>
                  {r.label}
                  {r.profile ? ` — ${r.profile.totalMs}ms` : ""}
                </option>
              ))}
          </select>
        </div>
      ) : null}
    </div>
  );
}
