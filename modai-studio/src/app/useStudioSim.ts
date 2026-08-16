import { useCallback, useEffect, useRef, useState } from "react";
import {
  cancelSimulation,
  emitArtifacts,
  listenSimProgress,
  pauseSimulation,
  resumeSimulation,
  simulateModel,
  stepSimulation,
  validateModel,
  type StudioProgressEvent,
} from "../api/studio";
import type {
  EmitOptions,
  EmitRecord,
  ExportMode,
  OpenProjectResult,
  RunRecord,
  SimProfile,
  SimulationResult,
  StudioProjectManifest,
  StudioReport,
} from "../api/types";
import { validateEdges } from "../canvas/editOps";
import {
  classifyStructureDiagnostics,
  findingsToDiagnostics,
  runStaticChecks,
} from "../advisor/staticChecks";
import { t, tf } from "../i18n";
import { localizeProgress } from "../i18n/progress";
import { collectPlotHintVars, pickSeriesByHints, resolveSeriesKeys } from "./plotHints";
import { collectParamSnapshot, mergeLiveSeries } from "../results/seriesDisplay";
import type { StudioEditSaveSnapshot } from "./useStudioEdit";

export type ResultsTab =
  | "results"
  | "data"
  | "messages"
  | "runs"
  | "xy"
  | "3d"
  | "profiler"
  | "export";

function modeToEmitFlags(mode: ExportMode, coderPackage: boolean) {
  if (mode === "coder") {
    return { packageFmu: false, coderPackage: true, fmiKind: "cs" };
  }
  if (mode === "fmu-me") {
    return { packageFmu: true, coderPackage, fmiKind: "me" };
  }
  return { packageFmu: true, coderPackage, fmiKind: "cs" };
}

type StudioSimDeps = {
  setStatus: (status: string) => void;
  rootDir: string | null;
  manifest: StudioProjectManifest;
  dirty: boolean;
  doSave: () => Promise<void>;
  getSaveSnapshot: () => StudioEditSaveSnapshot;
  effectiveRoot: StudioEditSaveSnapshot["diagram"];
  diagram: StudioEditSaveSnapshot["diagram"];
  bumpDiagramRev: () => void;
  setLoggedVariables?: (vars: string[]) => void;
};

export function useStudioSim(deps: StudioSimDeps) {
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<StudioReport | null>(null);
  const [result, setResult] = useState<SimulationResult | null>(null);
  const [selectedVars, setSelectedVars] = useState<string[]>([]);
  const [tab, setTab] = useState<ResultsTab>("results");
  const [runId, setRunId] = useState<string | null>(null);
  const [progress, setProgress] = useState<StudioProgressEvent | null>(null);
  const [paused, setPaused] = useState(false);
  const [runs, setRuns] = useState<RunRecord[]>([]);
  const [overlayRunIds, setOverlayRunIds] = useState<string[]>([]);
  const [xyX, setXyX] = useState("");
  const [xyY, setXyY] = useState("");
  const [fastRestart, setFastRestart] = useState(true);
  const [fullJitValidate, setFullJitValidate] = useState(false);
  const [lastProfile, setLastProfile] = useState<SimProfile | null>(null);
  const [profileRunId, setProfileRunId] = useState<string | null>(null);
  const [exportWizardOpen, setExportWizardOpen] = useState(false);
  const [exportPreset, setExportPreset] = useState<ExportMode>("coder");
  const [emitHistory, setEmitHistory] = useState<EmitRecord[]>([]);
  const [bgValidate, setBgValidate] = useState(false);
  const validateEpoch = useRef(0);

  const {
    setStatus,
    rootDir,
    manifest,
    dirty,
    doSave,
    getSaveSnapshot,
    effectiveRoot,
    diagram,
    bumpDiagramRev,
    setLoggedVariables,
  } = deps;

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listenSimProgress((ev) => {
      setProgress(ev);
      if (ev.runId) setRunId(ev.runId);
      if (ev.phase === "running" && ev.liveTime?.length) {
        const liveSeries = ev.liveSeries ?? {};
        setResult((prev) => mergeLiveSeries(prev, ev.liveTime!, liveSeries));
        setSelectedVars((prev) => {
          if (prev.length) return prev;
          return Object.keys(liveSeries)
            .filter((k) => k !== "time")
            .slice(0, 8);
        });
      }
      if (
        ev.phase === "running" ||
        ev.phase === "started" ||
        ev.phase === "fmu_cosim" ||
        ev.phase === "emit" ||
        ev.phase === "compile"
      ) {
        setStatus(localizeProgress(ev));
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [setStatus]);

  const resetProjectState = useCallback((p: OpenProjectResult) => {
    validateEpoch.current += 1;
    setBgValidate(false);
    setResult(null);
    setReport(null);
    const logged = p.manifest.simulationDefaults.loggedVariables ?? [];
    setSelectedVars(logged);
    setProgress(null);
    setRunId(null);
    setRuns([]);
    setOverlayRunIds([]);
    setPaused(false);
    setLastProfile(null);
    setProfileRunId(null);
    setEmitHistory([]);
    setExportWizardOpen(false);
  }, []);

  const cancelBgValidate = useCallback(() => {
    validateEpoch.current += 1;
    setBgValidate(false);
    setStatus(t("statusValidateCancelled"));
  }, [setStatus]);

  /** Background validate after an example paints; ignore stale results if cancelled. */
  const validateOpenedExample = useCallback(async (p: OpenProjectResult) => {
    const epoch = ++validateEpoch.current;
    setBgValidate(true);
    setTab("messages");
    setStatus(fullJitValidate ? t("statusValidatingFull") : t("statusValidating"));
    try {
      const invalid = validateEdges(p.diagram).filter((v) => !v.ok);
      const advisor = runStaticChecks(p.diagram, p.subsystems ?? {});
      const advisorDiags = findingsToDiagnostics(advisor);
      const r = await validateModel({
        rootDir: p.rootDir,
        modelName: p.manifest.modelName,
        diagram: p.diagram,
        subsystems: p.subsystems ?? {},
        libraryPaths: p.manifest.libraryPaths,
        options: {
          stopTime: p.manifest.simulationDefaults.stopTime,
          numberOfIntervals: p.manifest.simulationDefaults.numberOfIntervals,
          solver: p.manifest.simulationDefaults.solver,
          tolerance: p.manifest.simulationDefaults.tolerance,
        },
        emitFromDiagram: true,
        fullJit: fullJitValidate,
        paramDictionary: p.manifest.paramDictionary ?? [],
      });
      if (epoch !== validateEpoch.current) return;
      if (invalid.length) {
        r.warnings = [
          ...r.warnings,
          ...invalid.map((v) => ({
            message: v.reason ?? "invalid",
            source: "connection" as const,
            rule: v.reason ?? "connection",
            nodeId: p.diagram.edges.find((e) => e.id === v.id)?.sourceNodeId,
            severity: "warning" as const,
          })),
        ];
      }
      r.errors = [
        ...advisorDiags.errors,
        ...classifyStructureDiagnostics(
          r.errors.map((d) => ({ ...d, source: d.source ?? "compiler" })),
        ),
      ];
      r.warnings = [
        ...advisorDiags.warnings,
        ...classifyStructureDiagnostics(
          r.warnings.map((d) => ({ ...d, source: d.source ?? "compiler" })),
        ),
      ];
      if (advisorDiags.errors.length) r.success = false;
      if (r.success && r.stage === "validate_analyze") {
        r.warnings = [
          { message: t("validateAnalyzeOnlyHint") },
          ...r.warnings,
        ];
      }
      setReport(r);
      bumpDiagramRev();
      if (r.success) {
        const hints = collectPlotHintVars(p.diagram);
        setSelectedVars(hints.slice(0, 8));
        setTab("results");
        setStatus(tf("statusExampleValidated", { name: p.manifest.modelName, n: hints.length }));
      } else {
        setTab("messages");
        setStatus(t("statusValidateFailed"));
      }
    } catch (e) {
      if (epoch !== validateEpoch.current) return;
      setReport({
        success: false,
        stage: "validate",
        errors: [{ message: String(e) }],
        warnings: [],
      });
      setTab("messages");
      setStatus(t("statusValidateFailed"));
    } finally {
      if (epoch === validateEpoch.current) setBgValidate(false);
    }
  }, [bumpDiagramRev, fullJitValidate, setStatus]);

  const simOptions = () => ({
    stopTime: manifest.simulationDefaults.stopTime,
    numberOfIntervals: manifest.simulationDefaults.numberOfIntervals,
    solver: manifest.simulationDefaults.solver,
    tolerance: manifest.simulationDefaults.tolerance,
  });

  const doUpdate = async () => {
    if (!rootDir) {
      await doSave();
      setStatus(t("statusSaveProjectFirst"));
      return;
    }
    validateEpoch.current += 1;
    setBgValidate(false);
    setBusy(true);
    setTab("messages");
    setStatus(fullJitValidate ? t("statusValidatingFull") : t("statusValidating"));
    try {
      if (dirty) await doSave();
      const snap = getSaveSnapshot();
      const invalid = validateEdges(diagram).filter((v) => !v.ok);
      const advisor = runStaticChecks(snap.rootDiagram, snap.subsystems);
      const advisorDiags = findingsToDiagnostics(advisor);
      const r = await validateModel({
        rootDir: rootDir ?? undefined,
        modelName: manifest.modelName,
        diagram: snap.rootDiagram,
        subsystems: snap.subsystems,
        libraryPaths: manifest.libraryPaths,
        options: simOptions(),
        emitFromDiagram: true,
        fullJit: fullJitValidate,
        paramDictionary: manifest.paramDictionary ?? [],
      });
      if (invalid.length) {
        r.warnings = [
          ...r.warnings,
          ...invalid.map((v) => ({
            message: v.reason ?? "invalid",
            source: "connection" as const,
            rule: v.reason ?? "connection",
            nodeId: diagram.edges.find((e) => e.id === v.id)?.sourceNodeId,
            severity: "warning" as const,
          })),
        ];
      }
      r.errors = [
        ...advisorDiags.errors,
        ...classifyStructureDiagnostics(
          r.errors.map((d) => ({ ...d, source: d.source ?? "compiler" })),
        ),
      ];
      r.warnings = [
        ...advisorDiags.warnings,
        ...classifyStructureDiagnostics(
          r.warnings.map((d) => ({ ...d, source: d.source ?? "compiler" })),
        ),
      ];
      if (advisorDiags.errors.length) {
        r.success = false;
      }
      if (r.success && r.stage === "validate_analyze") {
        r.warnings = [
          { message: t("validateAnalyzeOnlyHint") },
          ...r.warnings,
        ];
      }
      setReport(r);
      bumpDiagramRev();
      if (r.success) {
        const nWarn = r.warnings.length;
        setStatus(
          r.stage === "validate_full"
            ? tf("statusValidateOkFullAdvisor", { n: nWarn })
            : tf("statusValidateOkAnalyzeAdvisor", { n: nWarn }),
        );
      } else {
        setStatus(t("statusValidateFailed"));
      }
    } catch (e) {
      setReport({
        success: false,
        stage: "validate",
        errors: [{ message: String(e) }],
        warnings: [],
      });
    } finally {
      setBusy(false);
    }
  };

  const doRun = async () => {
    if (!rootDir) {
      await doSave();
      setStatus(t("statusSaveProjectFirst"));
      return;
    }
    validateEpoch.current += 1;
    setBgValidate(false);
    setBusy(true);
    setPaused(false);
    setTab("results");
    setProgress(null);
    setResult({ time: [], series: {} });
    const t0 = performance.now();
    try {
      await doSave();
      const opts = simOptions();
      const { diagram: currentDiagram, rootDiagram, subsystems, viewId } = getSaveSnapshot();
      const resp = await simulateModel({
        rootDir: rootDir ?? undefined,
        modelName: manifest.modelName,
        diagram: viewId === "root" ? currentDiagram : rootDiagram,
        subsystems: viewId === "root" ? subsystems : { ...subsystems, [viewId]: currentDiagram },
        libraryPaths: manifest.libraryPaths,
        options: opts,
        emitFromDiagram: true,
        fastRestart,
        paramDictionary: manifest.paramDictionary ?? [],
      });
      if (resp.runId) setRunId(resp.runId);
      if (!resp.success || !resp.result) {
        setReport(
          resp.report ?? {
            success: false,
            stage: "simulate",
            errors: [
              {
                message: resp.cancelled ? t("errSimCancelled") : t("errSimFailed"),
              },
            ],
            warnings: [],
          },
        );
        setTab("messages");
        setStatus(resp.cancelled ? t("statusCancelled") : t("statusRunFailed"));
        return;
      }
      const wallMs = Math.round(performance.now() - t0);
      setResult(resp.result);
      setReport({ success: true, stage: "simulate", errors: [], warnings: [] });
      const modeLabel =
        resp.restartMode === "fastRestart"
          ? t("modeFastRestart")
          : resp.restartMode === "fullCompile"
            ? t("modeFullCompile")
            : t("modeRun");
      const profile = resp.profile ?? null;
      if (profile) {
        setLastProfile(profile);
        setProfileRunId(resp.runId ?? null);
      }
      setRuns((prev) => {
        const record: RunRecord = {
          runId: resp.runId ?? `run_${Date.now()}`,
          createdAt: new Date().toISOString(),
          label: `Run ${prev.length + 1} (${modeLabel})`,
          success: true,
          options: opts,
          result: resp.result!,
          wallMs,
          profile: profile ?? undefined,
          paramSnapshot: collectParamSnapshot(
            (viewId === "root" ? currentDiagram : rootDiagram).nodes,
            manifest.paramDictionary ?? [],
          ),
        };
        return [record, ...prev].slice(0, 20);
      });
      const hints = collectPlotHintVars(effectiveRoot);
      const keys = Object.keys(resp.result.series);
      const preferred = resolveSeriesKeys(keys, selectedVars);
      const picked = preferred.length
        ? preferred.slice(0, 8)
        : pickSeriesByHints(keys, hints);
      setSelectedVars(picked);
      if (!xyX && keys[0]) setXyX(keys[0]);
      if (!xyY && keys[1]) setXyY(keys[1]);
      setStatus(tf("statusRunOk", { ms: wallMs, mode: modeLabel }));
    } catch (e) {
      setReport({
        success: false,
        stage: "simulate",
        errors: [{ message: String(e) }],
        warnings: [],
      });
      setTab("messages");
      setStatus(t("statusRunError"));
    } finally {
      setBusy(false);
      setRunId(null);
    }
  };

  const openExportWizard = (preset: ExportMode = "coder") => {
    setExportPreset(preset);
    setExportWizardOpen(true);
  };

  const closeExportWizard = () => setExportWizardOpen(false);

  const doEmitWithOptions = async (opts: EmitOptions): Promise<boolean> => {
    if (!rootDir) {
      await doSave();
      setStatus(t("statusSaveProjectFirst"));
      return false;
    }
    const flags = modeToEmitFlags(opts.mode, opts.coderPackage);
    setBusy(true);
    setTab("messages");
    const t0 = performance.now();
    try {
      await doSave();
      const { diagram: currentDiagram, rootDiagram, subsystems, viewId } = getSaveSnapshot();
      const resp = await emitArtifacts({
        rootDir: rootDir ?? undefined,
        modelName: manifest.modelName,
        diagram: viewId === "root" ? currentDiagram : rootDiagram,
        subsystems: viewId === "root" ? subsystems : { ...subsystems, [viewId]: currentDiagram },
        libraryPaths: manifest.libraryPaths,
        options: simOptions(),
        outDir: opts.outDir,
        packageFmu: flags.packageFmu,
        coderPackage: flags.coderPackage,
        fmiKind: flags.fmiKind,
        modelIdentifier: opts.modelIdentifier,
        guid: opts.guid,
        emitFromDiagram: true,
        paramDictionary: manifest.paramDictionary ?? [],
      });
      const wallMs = Math.round(performance.now() - t0);
      const kind = resp.packageKind ?? (flags.packageFmu ? "fmu" : "coder");
      const record: EmitRecord = {
        id: `emit_${Date.now()}`,
        createdAt: new Date().toISOString(),
        modelName: manifest.modelName,
        mode: opts.mode,
        packageKind: kind,
        outDir: resp.outDir,
        fmuPath: resp.fmuPath,
        files: resp.files,
        success: resp.success,
        wallMs,
      };
      setEmitHistory((prev) => [record, ...prev].slice(0, 20));

      if (!resp.success) {
        setReport(
          resp.report ?? {
            success: false,
            stage: "emit",
            errors: [{ message: t("errEmitFailed") }],
            warnings: [],
          },
        );
        setStatus(t("statusEmitFailed"));
        setTab("messages");
        return false;
      }
      setReport({
        success: true,
        stage: "emit",
        errors: [],
        warnings: resp.files.map((f) => ({ message: f })),
      });
      setStatus(
        flags.packageFmu
          ? tf("statusEmitFmu", { kind, path: resp.fmuPath ?? resp.outDir })
          : tf("statusEmitCoder", {
              kind,
              dir: resp.outDir,
              count: resp.files.length,
            }),
      );
      setExportWizardOpen(false);
      setTab("export");
      return true;
    } catch (e) {
      setReport({
        success: false,
        stage: "emit",
        errors: [{ message: String(e) }],
        warnings: [],
      });
      setStatus(t("statusEmitError"));
      setTab("messages");
      return false;
    } finally {
      setBusy(false);
    }
  };

  const doStop = async () => {
    if (!runId) {
      if (bgValidate) cancelBgValidate();
      return;
    }
    const ok = await cancelSimulation(runId);
    setPaused(false);
    setStatus(ok ? t("statusCancelRequested") : t("statusNoActiveRun"));
  };

  const doPause = async () => {
    if (!runId) return;
    const ok = await pauseSimulation(runId);
    if (ok) {
      setPaused(true);
      setStatus(t("statusPaused"));
    }
  };

  const doResume = async () => {
    if (!runId) return;
    const ok = await resumeSimulation(runId);
    if (ok) {
      setPaused(false);
      setStatus(t("statusResumed"));
    }
  };

  const doStep = async () => {
    if (!runId) return;
    const ok = await stepSimulation(runId);
    if (ok) {
      setPaused(true);
      setStatus(t("statusStepOnce"));
    }
  };

  const toggleOverlayRun = (id: string) => {
    setOverlayRunIds((prev) =>
      prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id],
    );
  };

  const toggleSelectedVar = (name: string) => {
    setSelectedVars((prev) => {
      const next = prev.includes(name) ? prev.filter((x) => x !== name) : [...prev, name];
      setLoggedVariables?.(next);
      return next;
    });
  };

  const selectVars = (names: string[]) => {
    const uniq = [...new Set(names.filter(Boolean))];
    if (!uniq.length) return;
    const next = uniq.slice(0, 8);
    setSelectedVars(next);
    setLoggedVariables?.(next);
    setTab("results");
  };

  const selectProfileRun = (id: string) => {
    setProfileRunId(id);
    const hit = runs.find((r) => r.runId === id);
    if (hit?.profile) setLastProfile(hit.profile);
  };

  const progressLabel = progress ? localizeProgress(progress) : null;
  const progressFraction = progress?.fraction ?? null;
  const profileWallMs =
    (profileRunId ? runs.find((r) => r.runId === profileRunId)?.wallMs : undefined) ??
    runs.find((r) => r.profile)?.wallMs ??
    null;

  return {
    busy,
    setBusy,
    report,
    setReport,
    result,
    selectedVars,
    tab,
    setTab,
    runId,
    paused,
    runs,
    overlayRunIds,
    xyX,
    xyY,
    setXyX,
    setXyY,
    fastRestart,
    setFastRestart,
    fullJitValidate,
    setFullJitValidate,
    lastProfile,
    profileRunId,
    profileWallMs,
    selectProfileRun,
    resetProjectState,
    validateOpenedExample,
    bgValidate,
    cancelBgValidate,
    doUpdate,
    doRun,
    doStop,
    doPause,
    doResume,
    doStep,
    openExportWizard,
    closeExportWizard,
    exportWizardOpen,
    exportPreset,
    doEmitWithOptions,
    emitHistory,
    toggleOverlayRun,
    toggleSelectedVar,
    selectVars,
    progressLabel,
    progressFraction,
  };
}
