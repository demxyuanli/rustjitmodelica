import type { RunRecord } from "../api/types";
import { Timer } from "lucide-react";
import { t } from "../i18n";
import { IconButton } from "../shell/IconButton";
import { diffSnapshots } from "./seriesDisplay";

export function RunsPanel({
  runs,
  overlayRunIds,
  onToggleOverlay,
  onOpenProfiler,
}: {
  runs: RunRecord[];
  overlayRunIds: string[];
  onToggleOverlay: (runId: string) => void;
  onOpenProfiler: (runId: string) => void;
}) {
  if (!runs.length) {
    return <span style={{ color: "var(--muted)" }}>{t("resultsNoRuns")}</span>;
  }
  return (
    <div className="runs-panel">
      {runs.map((r, i) => {
        const older = runs[i + 1];
        const diffs = diffSnapshots(r.paramSnapshot, older?.paramSnapshot);
        return (
          <div key={r.runId} className="run-card">
            <label>
              <input
                type="checkbox"
                checked={overlayRunIds.includes(r.runId)}
                onChange={() => onToggleOverlay(r.runId)}
              />{" "}
              {r.label} {r.wallMs != null ? `(${r.wallMs}ms)` : ""}
            </label>
            {r.profile ? (
              <IconButton icon={Timer} labelKey="profiler" onClick={() => onOpenProfiler(r.runId)} />
            ) : null}
            <div className="run-snap">
              {diffs.length ? (
                diffs.slice(0, 8).map((d) => (
                  <div key={d.key}>
                    {d.key}: {d.from || "—"} → {d.to || "—"}
                  </div>
                ))
              ) : (
                <span style={{ color: "var(--muted)" }}>{t("runNoDiff")}</span>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
