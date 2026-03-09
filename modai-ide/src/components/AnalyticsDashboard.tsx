import { useState, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";

interface IterationRecord {
  id: number;
  target: string;
  diff: string | null;
  success: boolean;
  message: string;
  created_at: string;
  branch_name?: string | null;
  duration_ms?: number | null;
}

interface CoverageAnalysisResult {
  untestedFeatures: string[];
  uncoveredSources: string[];
  totalFeatures: number;
  coveredFeatures: number;
  totalSources: number;
  coveredSources: number;
  totalCases: number;
}

export function AnalyticsDashboard() {
  const [history, setHistory] = useState<IterationRecord[]>([]);
  const [coverage, setCoverage] = useState<CoverageAnalysisResult | null>(null);

  useEffect(() => {
    invoke<IterationRecord[]>("list_iteration_history", { limit: 200 }).then(setHistory).catch(() => {});
    invoke<CoverageAnalysisResult>("traceability_coverage_analysis").then(setCoverage).catch(() => {});
  }, []);

  const stats = useMemo(() => {
    const total = history.length;
    const passed = history.filter((h) => h.success).length;
    const failed = total - passed;
    const rate = total > 0 ? Math.round((passed / total) * 100) : 0;
    const durations = history.filter((h) => h.duration_ms).map((h) => h.duration_ms!);
    const avgDuration = durations.length > 0 ? Math.round(durations.reduce((a, b) => a + b, 0) / durations.length) : 0;
    return { total, passed, failed, rate, avgDuration };
  }, [history]);

  const recentTimeline = useMemo(() => {
    return history.slice(0, 20).reverse();
  }, [history]);

  const successByWeek = useMemo(() => {
    const weeks: Record<string, { passed: number; failed: number }> = {};
    for (const h of history) {
      const date = h.created_at.slice(0, 10);
      if (!weeks[date]) weeks[date] = { passed: 0, failed: 0 };
      if (h.success) weeks[date].passed++;
      else weeks[date].failed++;
    }
    return Object.entries(weeks).sort(([a], [b]) => a.localeCompare(b)).slice(-14);
  }, [history]);

  const featureCoveragePercent = coverage ? Math.round((coverage.coveredFeatures / Math.max(coverage.totalFeatures, 1)) * 100) : 0;
  const sourceCoveragePercent = coverage ? Math.round((coverage.coveredSources / Math.max(coverage.totalSources, 1)) * 100) : 0;

  return (
    <div className="flex flex-col h-full min-h-0 overflow-auto p-4">
      <h2 className="text-base font-semibold text-[var(--text)] mb-1">{t("analyticsTitle")}</h2>
      <p className="text-xs text-[var(--text-muted)] mb-4">{t("analyticsDesc")}</p>

      {/* Summary cards */}
      <div className="flex flex-wrap gap-3 mb-6">
        <div className="rounded-lg border border-gray-700 bg-[#2d2d2d] px-5 py-3 min-w-[120px]">
          <div className="text-[10px] uppercase text-[var(--text-muted)]">{t("totalIterations")}</div>
          <div className="text-2xl font-bold text-[var(--text)]">{stats.total}</div>
        </div>
        <div className="rounded-lg border border-green-700/50 bg-green-900/15 px-5 py-3 min-w-[120px]">
          <div className="text-[10px] uppercase text-green-400/80">{t("successRate")}</div>
          <div className="text-2xl font-bold text-green-300">{stats.rate}%</div>
          <div className="text-[10px] text-green-400/60">{stats.passed} passed / {stats.failed} failed</div>
        </div>
        <div className="rounded-lg border border-gray-700 bg-[#2d2d2d] px-5 py-3 min-w-[120px]">
          <div className="text-[10px] uppercase text-[var(--text-muted)]">{t("avgDuration")}</div>
          <div className="text-2xl font-bold text-[var(--text)]">{stats.avgDuration > 0 ? `${(stats.avgDuration / 1000).toFixed(1)}s` : "--"}</div>
        </div>
        {coverage && (
          <>
            <div className="rounded-lg border border-blue-700/50 bg-blue-900/15 px-5 py-3 min-w-[120px]">
              <div className="text-[10px] uppercase text-blue-400/80">Feature coverage</div>
              <div className="text-2xl font-bold text-blue-300">{featureCoveragePercent}%</div>
              <div className="text-[10px] text-blue-400/60">{coverage.coveredFeatures}/{coverage.totalFeatures}</div>
            </div>
            <div className="rounded-lg border border-amber-700/50 bg-amber-900/15 px-5 py-3 min-w-[120px]">
              <div className="text-[10px] uppercase text-amber-400/80">Source coverage</div>
              <div className="text-2xl font-bold text-amber-300">{sourceCoveragePercent}%</div>
              <div className="text-[10px] text-amber-400/60">{coverage.coveredSources}/{coverage.totalSources}</div>
            </div>
          </>
        )}
      </div>

      {/* Timeline bar chart */}
      {successByWeek.length > 0 && (
        <div className="rounded-lg border border-gray-700 bg-[#2d2d2d] p-4 mb-6">
          <h3 className="text-sm font-medium text-[var(--text)] mb-3">Iteration history by date</h3>
          <div className="flex items-end gap-1 h-24">
            {successByWeek.map(([date, data]) => {
              const total = data.passed + data.failed;
              const maxTotal = Math.max(...successByWeek.map(([, d]) => d.passed + d.failed), 1);
              const height = Math.max((total / maxTotal) * 100, 4);
              const passHeight = total > 0 ? (data.passed / total) * height : 0;
              const failHeight = height - passHeight;
              return (
                <div key={date} className="flex-1 flex flex-col items-center" title={`${date}: ${data.passed}P ${data.failed}F`}>
                  <div className="w-full flex flex-col justify-end" style={{ height: "80px" }}>
                    <div className="bg-red-500/60 rounded-t" style={{ height: `${failHeight}%` }} />
                    <div className="bg-green-500/60 rounded-b" style={{ height: `${passHeight}%` }} />
                  </div>
                  <div className="text-[8px] text-[var(--text-muted)] mt-1 rotate-[-45deg] origin-top-left w-10 truncate">{date.slice(5)}</div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Recent timeline */}
      <div className="rounded-lg border border-gray-700 bg-[#2d2d2d] p-4 mb-6">
        <h3 className="text-sm font-medium text-[var(--text)] mb-3">Recent iterations</h3>
        {recentTimeline.length === 0 ? (
          <div className="text-xs text-[var(--text-muted)]">No iterations yet</div>
        ) : (
          <div className="space-y-1">
            {recentTimeline.map((r) => (
              <div key={r.id} className="flex items-center gap-2 text-xs">
                <span className={`w-2 h-2 rounded-full shrink-0 ${r.success ? "bg-green-400" : "bg-red-400"}`} />
                <span className="text-[var(--text-muted)] w-28 shrink-0">{r.created_at.slice(0, 16)}</span>
                <span className="font-mono text-[var(--text-muted)] w-8 shrink-0">#{r.id}</span>
                <span className="text-[var(--text)] truncate flex-1">{r.target || "--"}</span>
                {r.duration_ms != null && (
                  <span className="text-[var(--text-muted)] shrink-0">{(r.duration_ms / 1000).toFixed(1)}s</span>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Coverage gaps */}
      {coverage && (coverage.untestedFeatures.length > 0 || coverage.uncoveredSources.length > 0) && (
        <div className="rounded-lg border border-amber-700/50 bg-amber-900/10 p-4">
          <h3 className="text-sm font-medium text-amber-300 mb-2">{t("coverageGaps")}</h3>
          {coverage.untestedFeatures.length > 0 && (
            <div className="mb-2">
              <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("untestedFeatures")}</div>
              <div className="flex flex-wrap gap-1">
                {coverage.untestedFeatures.map((f) => (
                  <span key={f} className="px-2 py-0.5 rounded bg-amber-900/40 text-amber-300 text-xs">{f}</span>
                ))}
              </div>
            </div>
          )}
          {coverage.uncoveredSources.length > 0 && (
            <div>
              <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("uncoveredSources")}</div>
              <div className="flex flex-wrap gap-1">
                {coverage.uncoveredSources.map((s) => (
                  <span key={s} className="px-2 py-0.5 rounded bg-red-900/40 text-red-300 text-xs font-mono">{s.replace("src/", "")}</span>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
