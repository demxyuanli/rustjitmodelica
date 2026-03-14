import { useState, useEffect, useMemo, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";

interface JitFeature {
  id: string;
  name: string;
  category: string;
  description: string;
  status: string;
}

interface RegressionCase {
  name: string;
  expected: string;
  notes?: string;
}

interface SourceModuleInfo {
  features: string[];
  description: string;
}

interface TraceabilityMatrix {
  features: JitFeature[];
  cases: RegressionCase[];
  featureToCases: Record<string, string[]>;
  caseToFeatures: Record<string, string[]>;
  sourceModules: Record<string, SourceModuleInfo>;
  caseToSourceFiles: Record<string, string[]>;
  sourceToFeatures: Record<string, string[]>;
  featureToSources: Record<string, string[]>;
  featureDependencies: Record<string, string[]>;
  featureDependents: Record<string, string[]>;
}

interface ImpactAnalysisResult {
  changedFiles: string[];
  affectedFeatures: string[];
  indirectlyAffectedFeatures: string[];
  affectedCases: string[];
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

interface SyncCheckResult {
  newSources: string[];
  removedSources: string[];
  newCases: string[];
  removedCases: string[];
}

interface ValidationError {
  kind: string;
  message: string;
  related: string[];
}

interface ValidationResult {
  errors: ValidationError[];
}

interface GitImpactResult {
  impact: ImpactAnalysisResult;
  suggestedCases: string[];
  unregisteredChangedSources: string[];
}

type ViewMode = "matrix" | "impact" | "coverage" | "sync" | "validate" | "gitImpact";

export function TraceabilityView() {
  const [matrix, setMatrix] = useState<TraceabilityMatrix | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>("matrix");
  const [selectedSource, setSelectedSource] = useState<string | null>(null);
  const [selectedFeature, setSelectedFeature] = useState<string | null>(null);
  const [selectedCase, setSelectedCase] = useState<string | null>(null);
  const [impactResult, setImpactResult] = useState<ImpactAnalysisResult | null>(null);
  const [coverageResult, setCoverageResult] = useState<CoverageAnalysisResult | null>(null);
  const [syncResult, setSyncResult] = useState<SyncCheckResult | null>(null);
  const [validationResult, setValidationResult] = useState<ValidationResult | null>(null);
  const [gitImpactResult, setGitImpactResult] = useState<GitImpactResult | null>(null);
  const [syncChecked, setSyncChecked] = useState<Set<string>>(new Set());
  const [applying, setApplying] = useState(false);

  const reloadMatrix = useCallback(() => {
    invoke<TraceabilityMatrix>("get_traceability_matrix").then(setMatrix).catch(() => {});
    invoke<CoverageAnalysisResult>("traceability_coverage_analysis").then(setCoverageResult).catch(() => {});
  }, []);

  useEffect(() => { reloadMatrix(); }, [reloadMatrix]);

  const sourceModules = useMemo(() => {
    if (!matrix) return [];
    return Object.keys(matrix.sourceModules).sort();
  }, [matrix]);

  const highlightedFeatures = useMemo(() => {
    if (!matrix) return new Set<string>();
    const s = new Set<string>();
    if (selectedSource && matrix.sourceToFeatures[selectedSource]) {
      matrix.sourceToFeatures[selectedSource].forEach((f) => s.add(f));
    }
    if (selectedCase && matrix.caseToFeatures[selectedCase]) {
      matrix.caseToFeatures[selectedCase].forEach((f) => s.add(f));
    }
    if (selectedFeature) s.add(selectedFeature);
    return s;
  }, [matrix, selectedSource, selectedFeature, selectedCase]);

  const highlightedCases = useMemo(() => {
    if (!matrix) return new Set<string>();
    const s = new Set<string>();
    if (selectedFeature && matrix.featureToCases[selectedFeature]) {
      matrix.featureToCases[selectedFeature].forEach((c) => s.add(c));
    }
    if (selectedSource) {
      for (const [caseName, sources] of Object.entries(matrix.caseToSourceFiles)) {
        if (sources.includes(selectedSource)) s.add(caseName);
      }
    }
    if (selectedCase) s.add(selectedCase);
    return s;
  }, [matrix, selectedSource, selectedFeature, selectedCase]);

  const highlightedSources = useMemo(() => {
    if (!matrix) return new Set<string>();
    const s = new Set<string>();
    if (selectedFeature && matrix.featureToSources[selectedFeature]) {
      matrix.featureToSources[selectedFeature].forEach((src) => s.add(src));
    }
    if (selectedCase && matrix.caseToSourceFiles[selectedCase]) {
      matrix.caseToSourceFiles[selectedCase].forEach((src) => s.add(src));
    }
    if (selectedSource) s.add(selectedSource);
    return s;
  }, [matrix, selectedSource, selectedFeature, selectedCase]);

  const clearSelection = useCallback(() => {
    setSelectedSource(null);
    setSelectedFeature(null);
    setSelectedCase(null);
  }, []);

  const runImpactAnalysis = useCallback(async () => {
    if (highlightedSources.size === 0) return;
    try {
      const result = await invoke<ImpactAnalysisResult>("traceability_impact_analysis", {
        changedFiles: Array.from(highlightedSources),
      });
      setImpactResult(result);
      setViewMode("impact");
    } catch {}
  }, [highlightedSources]);

  const runSyncCheck = useCallback(async () => {
    try {
      const result = await invoke<SyncCheckResult>("traceability_sync_check");
      setSyncResult(result);
      setSyncChecked(new Set());
      setViewMode("sync");
    } catch {}
  }, []);

  const runValidation = useCallback(async () => {
    try {
      const result = await invoke<ValidationResult>("traceability_validate");
      setValidationResult(result);
      setViewMode("validate");
    } catch {}
  }, []);

  const runGitImpact = useCallback(async () => {
    try {
      const result = await invoke<GitImpactResult>("traceability_git_impact");
      setGitImpactResult(result);
      setViewMode("gitImpact");
    } catch {}
  }, []);

  const toggleSyncItem = useCallback((key: string) => {
    setSyncChecked((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }, []);

  const applySync = useCallback(async () => {
    if (!syncResult || syncChecked.size === 0) return;
    setApplying(true);
    try {
      const addSources = syncResult.newSources.filter((s) => syncChecked.has("src:" + s));
      const removeSources = syncResult.removedSources.filter((s) => syncChecked.has("rsrc:" + s));
      const addCases = syncResult.newCases.filter((c) => syncChecked.has("case:" + c));
      const removeCases = syncResult.removedCases.filter((c) => syncChecked.has("rcase:" + c));
      await invoke("traceability_apply_sync", {
        request: { addSources, removeSources, addCases, removeCases },
      });
      reloadMatrix();
      const fresh = await invoke<SyncCheckResult>("traceability_sync_check");
      setSyncResult(fresh);
      setSyncChecked(new Set());
    } catch {} finally {
      setApplying(false);
    }
  }, [syncResult, syncChecked, reloadMatrix]);

  if (!matrix) {
    return <div className="flex items-center justify-center h-full text-sm text-[var(--text-muted)]">{t("loading")}</div>;
  }

  const viewModes: { id: ViewMode; label: string }[] = [
    { id: "matrix", label: t("matrix") },
    { id: "coverage", label: t("coverageAnalysis") },
    { id: "sync", label: t("syncCheck") },
    { id: "validate", label: t("validateConfig") },
    { id: "gitImpact", label: t("gitImpact") },
    { id: "impact", label: t("impactAnalysis") },
  ];

  return (
    <div className="flex flex-col h-full min-h-0 overflow-hidden p-4">
      <div className="flex items-center justify-between mb-3 shrink-0">
        <h2 className="text-base font-semibold text-[var(--text)]">{t("traceabilityTitle")}</h2>
        <div className="flex gap-1.5 flex-wrap">
          {viewModes.map((m) => (
            <button
              key={m.id}
              type="button"
              onClick={() => {
                if (m.id === "sync") runSyncCheck();
                else if (m.id === "validate") runValidation();
                else if (m.id === "gitImpact") runGitImpact();
                else setViewMode(m.id);
              }}
              className={`px-3 py-1 text-xs rounded-lg border ${viewMode === m.id ? "bg-primary/20 text-primary border-primary/30" : "theme-button-secondary text-[var(--text-muted)]"}`}
            >
              {m.label}
            </button>
          ))}
          <button type="button" onClick={clearSelection} className="px-3 py-1 text-xs rounded-lg border theme-button-secondary text-[var(--text-muted)]">
            {t("clear")}
          </button>
          {highlightedSources.size > 0 && viewMode === "matrix" && (
            <button type="button" onClick={runImpactAnalysis} className="px-3 py-1 text-xs rounded-lg border theme-banner-warning">
              {t("impactAnalysis")}
            </button>
          )}
        </div>
      </div>

      {viewMode === "matrix" && (
        <div className="flex flex-1 min-h-0 gap-3 overflow-hidden">
          <div className="w-1/3 rounded-lg border border-border bg-[var(--surface-elevated)] overflow-hidden flex flex-col">
            <div className="px-3 py-2 text-xs font-medium text-[var(--text-muted)] bg-[var(--panel-muted-bg)] border-b border-border shrink-0">
              {t("sourceModules")} ({sourceModules.length})
            </div>
            <div className="flex-1 overflow-auto">
              {sourceModules.map((src) => {
                const hl = highlightedSources.has(src);
                const info = matrix.sourceModules[src];
                return (
                  <button
                    key={src}
                    type="button"
                    className={`w-full text-left px-3 py-1.5 text-xs border-b border-border/40 ${hl ? "bg-primary/15 text-primary" : "hover:bg-[var(--surface-hover)] text-[var(--text)]"}`}
                    onClick={() => {
                      setSelectedSource(src === selectedSource ? null : src);
                      setSelectedFeature(null);
                      setSelectedCase(null);
                    }}
                  >
                    <div className="font-mono truncate">{src.replace("src/", "")}</div>
                    {info && <div className="text-[10px] text-[var(--text-muted)] truncate">{info.description}</div>}
                  </button>
                );
              })}
            </div>
          </div>

          <div className="w-1/3 rounded-lg border border-border bg-[var(--surface-elevated)] overflow-hidden flex flex-col">
            <div className="px-3 py-2 text-xs font-medium text-[var(--text-muted)] bg-[var(--panel-muted-bg)] border-b border-border shrink-0">
              {t("linkedFeatures")} ({matrix.features.length})
            </div>
            <div className="flex-1 overflow-auto">
              {matrix.features.map((f) => {
                const hl = highlightedFeatures.has(f.id);
                const deps = matrix.featureDependencies[f.id];
                const depnts = matrix.featureDependents[f.id];
                return (
                  <button
                    key={f.id}
                    type="button"
                    className={`w-full text-left px-3 py-1.5 text-xs border-b border-border/40 ${hl ? "bg-primary/20 text-primary" : "hover:bg-[var(--surface-hover)] text-[var(--text)]"}`}
                    onClick={() => {
                      setSelectedFeature(f.id === selectedFeature ? null : f.id);
                      setSelectedSource(null);
                      setSelectedCase(null);
                    }}
                  >
                    <div className="flex items-center gap-1">
                      <span className="font-mono text-[10px] text-[var(--text-muted)] w-10 shrink-0">{f.id}</span>
                      <span className="truncate">{f.name}</span>
                      <span className={`ml-auto text-[9px] px-1.5 py-0.5 rounded shrink-0 ${f.status === "covered" ? "theme-banner-success" : "theme-banner-warning"}`}>
                        {f.status}
                      </span>
                    </div>
                    {selectedFeature === f.id && (deps?.length || depnts?.length) ? (
                      <div className="mt-1 flex flex-wrap gap-1">
                        {deps?.map((d) => (
                          <span key={d} className="px-1 py-0.5 rounded theme-banner-info text-[9px]">{t("featureDeps")}: {d}</span>
                        ))}
                        {depnts?.map((d) => (
                          <span key={d} className="px-1 py-0.5 rounded theme-banner-info text-[9px]">{t("featureDependents")}: {d}</span>
                        ))}
                      </div>
                    ) : null}
                  </button>
                );
              })}
            </div>
          </div>

          <div className="w-1/3 rounded-lg border border-border bg-[var(--surface-elevated)] overflow-hidden flex flex-col">
            <div className="px-3 py-2 text-xs font-medium text-[var(--text-muted)] bg-[var(--panel-muted-bg)] border-b border-border shrink-0">
              {t("testCases")} ({matrix.cases.length})
            </div>
            <div className="flex-1 overflow-auto">
              {matrix.cases.map((c) => {
                const hl = highlightedCases.has(c.name);
                return (
                  <button
                    key={c.name}
                    type="button"
                    className={`w-full text-left px-3 py-1.5 text-xs border-b border-border/40 ${hl ? "bg-[var(--success-bg)] text-[var(--success-text)]" : "hover:bg-[var(--surface-hover)] text-[var(--text)]"}`}
                    onClick={() => {
                      setSelectedCase(c.name === selectedCase ? null : c.name);
                      setSelectedSource(null);
                      setSelectedFeature(null);
                    }}
                  >
                    <div className="flex items-center gap-1">
                      <span className="truncate">{c.name.replace("TestLib/", "")}</span>
                      <span className={`ml-auto text-[9px] px-1.5 py-0.5 rounded shrink-0 ${c.expected === "pass" ? "theme-banner-success" : "theme-banner-danger"}`}>
                        {c.expected}
                      </span>
                    </div>
                  </button>
                );
              })}
            </div>
          </div>
        </div>
      )}

      {viewMode === "impact" && impactResult && (
        <div className="flex-1 overflow-auto">
          <div className="rounded-lg border border-border bg-[var(--surface-elevated)] p-4 mb-3">
            <h3 className="text-sm font-medium text-[var(--text)] mb-2">{t("impactAnalysis")}</h3>
            <div className="mb-3">
              <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("changedFiles")}</div>
              <div className="flex flex-wrap gap-1">
                {impactResult.changedFiles.map((f) => (
                  <span key={f} className="px-2 py-0.5 rounded theme-banner-warning text-xs font-mono">{f.replace("src/", "")}</span>
                ))}
              </div>
            </div>
            <div className="mb-3">
              <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("affectedFeatures")} ({impactResult.affectedFeatures.length})</div>
              <div className="flex flex-wrap gap-1">
                {impactResult.affectedFeatures.map((f) => (
                  <span key={f} className="px-2 py-0.5 rounded theme-banner-info text-xs">{f}</span>
                ))}
              </div>
            </div>
            {impactResult.indirectlyAffectedFeatures.length > 0 && (
              <div className="mb-3">
                <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("indirectlyAffected")} ({impactResult.indirectlyAffectedFeatures.length})</div>
                <div className="flex flex-wrap gap-1">
                  {impactResult.indirectlyAffectedFeatures.map((f) => (
                    <span key={f} className="px-2 py-0.5 rounded theme-banner-info text-xs">{f}</span>
                  ))}
                </div>
              </div>
            )}
            <div>
              <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("affectedTests")} ({impactResult.affectedCases.length})</div>
              <div className="flex flex-wrap gap-1">
                {impactResult.affectedCases.map((c) => (
                  <span key={c} className="px-2 py-0.5 rounded theme-banner-success text-xs">{c.replace("TestLib/", "")}</span>
                ))}
              </div>
            </div>
          </div>
        </div>
      )}

      {viewMode === "coverage" && coverageResult && (
        <div className="flex-1 overflow-auto">
          <div className="flex flex-wrap gap-3 mb-4">
            <div className="rounded-lg border border-border bg-[var(--surface-elevated)] px-4 py-2 min-w-[100px]">
              <div className="text-[10px] uppercase text-[var(--text-muted)]">{t("linkedFeatures")}</div>
              <div className="text-lg font-semibold text-[var(--text)]">{coverageResult.coveredFeatures}/{coverageResult.totalFeatures}</div>
            </div>
            <div className="rounded-lg border border-border bg-[var(--surface-elevated)] px-4 py-2 min-w-[100px]">
              <div className="text-[10px] uppercase text-[var(--text-muted)]">{t("linkedSources")}</div>
              <div className="text-lg font-semibold text-[var(--text)]">{coverageResult.coveredSources}/{coverageResult.totalSources}</div>
            </div>
            <div className="rounded-lg border border-border bg-[var(--surface-elevated)] px-4 py-2 min-w-[100px]">
              <div className="text-[10px] uppercase text-[var(--text-muted)]">{t("testCases")}</div>
              <div className="text-lg font-semibold text-[var(--text)]">{coverageResult.totalCases}</div>
            </div>
          </div>
          {coverageResult.untestedFeatures.length > 0 && (
            <div className="rounded-lg border theme-banner-warning p-4 mb-3">
              <div className="text-xs font-medium mb-2">{t("untestedFeatures")} ({coverageResult.untestedFeatures.length})</div>
              <div className="flex flex-wrap gap-1">
                {coverageResult.untestedFeatures.map((f) => (
                  <span key={f} className="px-2 py-0.5 rounded theme-banner-warning text-xs">{f}</span>
                ))}
              </div>
            </div>
          )}
          {coverageResult.uncoveredSources.length > 0 && (
            <div className="rounded-lg border theme-banner-danger p-4">
              <div className="text-xs font-medium text-red-300 mb-2">{t("uncoveredSources")} ({coverageResult.uncoveredSources.length})</div>
              <div className="flex flex-wrap gap-1">
                {coverageResult.uncoveredSources.map((s) => (
                  <span key={s} className="px-2 py-0.5 rounded theme-banner-danger text-xs font-mono">{s.replace("src/", "")}</span>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {viewMode === "sync" && (
        <div className="flex-1 overflow-auto">
          {syncResult && (syncResult.newSources.length + syncResult.removedSources.length + syncResult.newCases.length + syncResult.removedCases.length) === 0 ? (
            <div className="rounded-lg border theme-banner-success p-4">
              <div className="text-xs font-medium">{t("noSyncIssues")}</div>
            </div>
          ) : syncResult ? (
            <div className="flex flex-col gap-3">
              {syncResult.newSources.length > 0 && (
                <div className="rounded-lg border theme-banner-info p-4">
                  <div className="text-xs font-medium mb-2">{t("newSources")} ({syncResult.newSources.length})</div>
                  <div className="flex flex-col gap-1">
                    {syncResult.newSources.map((s) => {
                      const key = "src:" + s;
                      return (
                        <label key={s} className="flex items-center gap-2 text-xs text-[var(--text)] cursor-pointer hover:bg-[var(--surface-hover)] rounded px-1 py-0.5">
                          <input type="checkbox" checked={syncChecked.has(key)} onChange={() => toggleSyncItem(key)} className="accent-primary" />
                          <span className="font-mono">{s}</span>
                        </label>
                      );
                    })}
                  </div>
                </div>
              )}
              {syncResult.removedSources.length > 0 && (
                <div className="rounded-lg border theme-banner-danger p-4">
                  <div className="text-xs font-medium mb-2">{t("removedSources")} ({syncResult.removedSources.length})</div>
                  <div className="flex flex-col gap-1">
                    {syncResult.removedSources.map((s) => {
                      const key = "rsrc:" + s;
                      return (
                        <label key={s} className="flex items-center gap-2 text-xs text-[var(--text)] cursor-pointer hover:bg-[var(--surface-hover)] rounded px-1 py-0.5">
                          <input type="checkbox" checked={syncChecked.has(key)} onChange={() => toggleSyncItem(key)} className="accent-primary" />
                          <span className="font-mono line-through text-[var(--danger-text)]">{s}</span>
                        </label>
                      );
                    })}
                  </div>
                </div>
              )}
              {syncResult.newCases.length > 0 && (
                <div className="rounded-lg border theme-banner-info p-4">
                  <div className="text-xs font-medium mb-2">{t("newCases")} ({syncResult.newCases.length})</div>
                  <div className="flex flex-col gap-1">
                    {syncResult.newCases.map((c) => {
                      const key = "case:" + c;
                      return (
                        <label key={c} className="flex items-center gap-2 text-xs text-[var(--text)] cursor-pointer hover:bg-[var(--surface-hover)] rounded px-1 py-0.5">
                          <input type="checkbox" checked={syncChecked.has(key)} onChange={() => toggleSyncItem(key)} className="accent-primary" />
                          <span>{c}</span>
                        </label>
                      );
                    })}
                  </div>
                </div>
              )}
              {syncResult.removedCases.length > 0 && (
                <div className="rounded-lg border theme-banner-danger p-4">
                  <div className="text-xs font-medium mb-2">{t("removedCases")} ({syncResult.removedCases.length})</div>
                  <div className="flex flex-col gap-1">
                    {syncResult.removedCases.map((c) => {
                      const key = "rcase:" + c;
                      return (
                        <label key={c} className="flex items-center gap-2 text-xs text-[var(--text)] cursor-pointer hover:bg-[var(--surface-hover)] rounded px-1 py-0.5">
                          <input type="checkbox" checked={syncChecked.has(key)} onChange={() => toggleSyncItem(key)} className="accent-primary" />
                          <span className="line-through text-[var(--danger-text)]">{c}</span>
                        </label>
                      );
                    })}
                  </div>
                </div>
              )}
              {syncChecked.size > 0 && (
                <div className="shrink-0 flex gap-2">
                  <button
                    type="button"
                    onClick={applySync}
                    disabled={applying}
                    className="px-4 py-1.5 text-xs rounded bg-primary hover:bg-blue-600 text-white disabled:opacity-50"
                  >
                    {applying ? t("running") : `${t("registerSelected")} (${syncChecked.size})`}
                  </button>
                </div>
              )}
            </div>
          ) : (
            <div className="text-sm text-[var(--text-muted)]">{t("loading")}</div>
          )}
        </div>
      )}

      {viewMode === "validate" && (
        <div className="flex-1 overflow-auto">
          {validationResult && validationResult.errors.length === 0 ? (
            <div className="rounded-lg border theme-banner-success p-4">
              <div className="text-xs font-medium">{t("noValidationErrors")}</div>
            </div>
          ) : validationResult ? (
            <div className="rounded-lg border theme-banner-danger p-4">
              <div className="text-xs font-medium mb-2">{t("validationErrors")} ({validationResult.errors.length})</div>
              <div className="flex flex-col gap-1.5">
                {validationResult.errors.map((e, i) => (
                  <div key={i} className="flex items-start gap-2 text-xs">
                    <span className={`shrink-0 px-1.5 py-0.5 rounded text-[9px] font-mono ${
                      e.kind === "orphan_feature" ? "theme-banner-warning" : "theme-banner-danger"
                    }`}>
                      {e.kind}
                    </span>
                    <span className="text-[var(--text)]">{e.message}</span>
                  </div>
                ))}
              </div>
            </div>
          ) : (
            <div className="text-sm text-[var(--text-muted)]">{t("loading")}</div>
          )}
        </div>
      )}

      {viewMode === "gitImpact" && (
        <div className="flex-1 overflow-auto">
          {gitImpactResult ? (
            <div className="flex flex-col gap-3">
              {gitImpactResult.impact.changedFiles.length === 0 ? (
                <div className="rounded-lg border theme-banner-success p-4">
                  <div className="text-xs font-medium">{t("noChangedSourceFilesInGit")}</div>
                </div>
              ) : (
                <>
                  <div className="rounded-lg border border-border bg-[var(--surface-elevated)] p-4">
                    <div className="mb-3">
                      <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("changedFiles")} ({gitImpactResult.impact.changedFiles.length})</div>
                      <div className="flex flex-wrap gap-1">
                        {gitImpactResult.impact.changedFiles.map((f) => (
                          <span key={f} className="px-2 py-0.5 rounded theme-banner-warning text-xs font-mono">{f.replace("src/", "")}</span>
                        ))}
                      </div>
                    </div>
                    {gitImpactResult.impact.affectedFeatures.length > 0 && (
                      <div className="mb-3">
                        <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("affectedFeatures")} ({gitImpactResult.impact.affectedFeatures.length})</div>
                        <div className="flex flex-wrap gap-1">
                          {gitImpactResult.impact.affectedFeatures.map((f) => (
                            <span key={f} className="px-2 py-0.5 rounded theme-banner-info text-xs">{f}</span>
                          ))}
                        </div>
                      </div>
                    )}
                    {gitImpactResult.impact.indirectlyAffectedFeatures.length > 0 && (
                      <div className="mb-3">
                        <div className="text-[10px] uppercase text-[var(--text-muted)] mb-1">{t("indirectlyAffected")} ({gitImpactResult.impact.indirectlyAffectedFeatures.length})</div>
                        <div className="flex flex-wrap gap-1">
                          {gitImpactResult.impact.indirectlyAffectedFeatures.map((f) => (
                            <span key={f} className="px-2 py-0.5 rounded theme-banner-info text-xs">{f}</span>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                  {gitImpactResult.suggestedCases.length > 0 && (
                    <div className="rounded-lg border theme-banner-success p-4">
                      <div className="text-xs font-medium mb-2">{t("suggestedCases")} ({gitImpactResult.suggestedCases.length})</div>
                      <div className="flex flex-wrap gap-1">
                        {gitImpactResult.suggestedCases.map((c) => (
                          <span key={c} className="px-2 py-0.5 rounded theme-banner-success text-xs">{c.replace("TestLib/", "")}</span>
                        ))}
                      </div>
                    </div>
                  )}
                  {gitImpactResult.unregisteredChangedSources.length > 0 && (
                    <div className="rounded-lg border theme-banner-warning p-4">
                      <div className="text-xs font-medium mb-2">{t("unregisteredChanged")} ({gitImpactResult.unregisteredChangedSources.length})</div>
                      <div className="flex flex-wrap gap-1">
                        {gitImpactResult.unregisteredChangedSources.map((s) => (
                          <span key={s} className="px-2 py-0.5 rounded theme-banner-warning text-xs font-mono">{s}</span>
                        ))}
                      </div>
                    </div>
                  )}
                </>
              )}
            </div>
          ) : (
            <div className="text-sm text-[var(--text-muted)]">{t("loading")}</div>
          )}
        </div>
      )}
    </div>
  );
}
