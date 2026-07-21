import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  cancelSimulation,
  catalogList,
  emitArtifacts,
  importFmu,
  listenSimProgress,
  pickDirectory,
  pickExportDirectory,
  pickFmuFile,
  projectCreate,
  projectOpen,
  projectSave,
  simulateModel,
  validateModel,
  type StudioProgressEvent,
} from "../api/studio";
import type {
  CatalogEntry,
  DiagramDocument,
  DiagramNode,
  OpenProjectResult,
  RunRecord,
  SimulationResult,
  StudioProjectManifest,
  StudioReport,
} from "../api/types";
import { StudioCanvas } from "../canvas/StudioCanvas";
import {
  alignSelection,
  copySelection,
  extractInstanceHint,
  pasteClipboard,
  validateEdges,
} from "../canvas/editOps";
import { createSubsystemFromSelection } from "../canvas/subsystemOps";
import { DashboardPanel } from "../dashboard/DashboardPanel";
import { LibraryPanel } from "../library/LibraryPanel";
import { OutlinePanel } from "../outline/OutlinePanel";
import { PropertiesPanel } from "../properties/PropertiesPanel";
import { ResultsDock } from "../results/ResultsDock";
import { useDiagramHistory } from "../project/useDiagramHistory";
import { exportResultsCsv, exportResultsJson } from "../results/exportResults";

function emptySession(modelName: string): {
  rootDir: string | null;
  manifest: StudioProjectManifest;
  diagram: DiagramDocument;
} {
  return {
    rootDir: null,
    manifest: {
      schemaVersion: 1,
      product: "modai-studio",
      modelName,
      libraryPaths: [],
      simulationDefaults: {
        stopTime: 1,
        numberOfIntervals: 500,
        solver: "rk45",
        tolerance: 1e-6,
        loggedVariables: [],
      },
      ui: { locale: "zh" },
    },
    diagram: {
      schemaVersion: 1,
      modelName,
      diagramId: "root",
      parentDiagramId: null,
      nodes: [],
      edges: [],
      annotations: [],
      viewport: { x: 0, y: 0, zoom: 1 },
    },
  };
}

export function App() {
  const boot = useMemo(() => emptySession("Plant"), []);
  const [rootDir, setRootDir] = useState<string | null>(boot.rootDir);
  const [manifest, setManifest] = useState(boot.manifest);
  const [rootDiagram, setRootDiagram] = useState(boot.diagram);
  const [subsystems, setSubsystems] = useState<Record<string, DiagramDocument>>({});
  const [viewStack, setViewStack] = useState<string[]>(["root"]);
  const { diagram, commit, replace, undo, redo } = useDiagramHistory(boot.diagram);
  const [diagramRev, setDiagramRev] = useState(0);
  const [dirty, setDirty] = useState(false);
  const [catalog, setCatalog] = useState<CatalogEntry[]>([]);
  const [filter, setFilter] = useState("");
  const [pending, setPending] = useState<CatalogEntry | null>(null);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<StudioReport | null>(null);
  const [result, setResult] = useState<SimulationResult | null>(null);
  const [selectedVars, setSelectedVars] = useState<string[]>([]);
  const [tab, setTab] = useState<"results" | "messages" | "runs" | "xy">("results");
  const [status, setStatus] = useState("Ready");
  const [runId, setRunId] = useState<string | null>(null);
  const [progress, setProgress] = useState<StudioProgressEvent | null>(null);
  const [snapEnabled, setSnapEnabled] = useState(true);
  const [focusNodeId, setFocusNodeId] = useState<string | null>(null);
  const [runs, setRuns] = useState<RunRecord[]>([]);
  const [overlayRunIds, setOverlayRunIds] = useState<string[]>([]);
  const [xyX, setXyX] = useState("");
  const [xyY, setXyY] = useState("");
  const [showDashboard, setShowDashboard] = useState(true);
  const clipboard = useRef<{ nodes: DiagramNode[]; edges: import("../api/types").DiagramEdge[] } | null>(
    null,
  );

  const viewId = viewStack[viewStack.length - 1] ?? "root";

  useEffect(() => {
    catalogList(rootDir).then(setCatalog).catch((e: unknown) => setStatus(String(e)));
  }, [rootDir]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listenSimProgress((ev) => {
      setProgress(ev);
      if (ev.runId) setRunId(ev.runId);
      if (ev.phase === "running" || ev.phase === "started") setStatus(ev.message);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  const persistView = useCallback(
    (d: DiagramDocument, id: string) => {
      if (id === "root") setRootDiagram(d);
      else setSubsystems((prev) => ({ ...prev, [id]: d }));
    },
    [],
  );

  const loadProject = useCallback(
    (p: OpenProjectResult) => {
      setRootDir(p.rootDir);
      setManifest(p.manifest);
      setRootDiagram(p.diagram);
      setSubsystems(p.subsystems ?? {});
      setViewStack(["root"]);
      replace(p.diagram);
      setDiagramRev((x) => x + 1);
      setDirty(false);
      setResult(null);
      setReport(null);
      setSelectedVars([]);
      setProgress(null);
      setRunId(null);
      setRuns([]);
      setOverlayRunIds([]);
      setStatus(`Opened ${p.rootDir}`);
    },
    [replace],
  );

  const updateDiagram = useCallback(
    (d: DiagramDocument) => {
      commit(d, true);
      persistView(d, viewId);
      setDirty(true);
    },
    [commit, persistView, viewId],
  );

  const replaceDiagram = useCallback(
    (d: DiagramDocument) => {
      commit(d, true);
      persistView(d, viewId);
      setDiagramRev((x) => x + 1);
      setDirty(true);
    },
    [commit, persistView, viewId],
  );

  const doUndo = () => {
    if (undo()) {
      setDiagramRev((x) => x + 1);
      setDirty(true);
      setStatus("Undo");
    }
  };

  const doRedo = () => {
    if (redo()) {
      setDiagramRev((x) => x + 1);
      setDirty(true);
      setStatus("Redo");
    }
  };

  const selectedNode: DiagramNode | null = useMemo(() => {
    const id = selectedIds[0];
    if (!id) return null;
    return diagram.nodes.find((n) => n.id === id) ?? null;
  }, [selectedIds, diagram.nodes]);

  const onNodePropChange = useCallback(
    (n: DiagramNode) => {
      replaceDiagram({
        ...diagram,
        nodes: diagram.nodes.map((x) => (x.id === n.id ? n : x)),
      });
    },
    [diagram, replaceDiagram],
  );

  const simOptions = () => ({
    stopTime: manifest.simulationDefaults.stopTime,
    numberOfIntervals: manifest.simulationDefaults.numberOfIntervals,
    solver: manifest.simulationDefaults.solver,
    tolerance: manifest.simulationDefaults.tolerance,
  });

  const effectiveRoot = viewId === "root" ? diagram : rootDiagram;
  const effectiveSubs =
    viewId === "root"
      ? subsystems
      : { ...subsystems, [viewId]: diagram };

  const doSave = async () => {
    const root = viewId === "root" ? diagram : rootDiagram;
    const subs = viewId === "root" ? subsystems : { ...subsystems, [viewId]: diagram };
    if (!rootDir) {
      const dir = await pickDirectory();
      if (!dir) return;
      const created = await projectCreate(dir, manifest.modelName);
      loadProject(created);
      await projectSave(dir, { ...manifest, modelName: created.manifest.modelName }, root, subs);
      setRootDir(dir);
      setDirty(false);
      setStatus("Saved");
      return;
    }
    await projectSave(rootDir, manifest, root, subs);
    setRootDiagram(root);
    setSubsystems(subs);
    setDirty(false);
    setStatus("Saved");
  };

  const doNew = async () => {
    const dir = await pickDirectory();
    if (!dir) return;
    const created = await projectCreate(dir, "Plant");
    loadProject(created);
  };

  const doOpen = async () => {
    const dir = await pickDirectory();
    if (!dir) return;
    const opened = await projectOpen(dir);
    loadProject(opened);
  };

  const doImportFmu = async () => {
    const path = await pickFmuFile();
    if (!path) return;
    setBusy(true);
    try {
      const res = await importFmu({ fmuPath: path, projectRoot: rootDir });
      if (!res.success || !res.catalogEntry) {
        setStatus(res.error ?? "FMU import failed");
        setReport({
          success: false,
          stage: "fmu_import",
          errors: [{ message: res.error ?? "FMU import failed" }],
          warnings: [],
        });
        setTab("messages");
        return;
      }
      setCatalog((prev) => {
        const rest = prev.filter((c) => c.id !== res.catalogEntry!.id);
        return [...rest, res.catalogEntry!];
      });
      setPending(res.catalogEntry);
      const binNote = res.hasBinary ? "binary OK" : "no binary (will try compile at Run)";
      setStatus(
        `Imported FMU ${res.modelIdentifier} (${binNote}). Click canvas to place.`,
      );
    } catch (e) {
      setStatus(String(e));
    } finally {
      setBusy(false);
    }
  };

  const doUpdate = async () => {
    setBusy(true);
    setTab("messages");
    try {
      if (dirty && rootDir) await doSave();
      const invalid = validateEdges(diagram).filter((v) => !v.ok);
      const r = await validateModel({
        rootDir: rootDir ?? undefined,
        modelName: manifest.modelName,
        diagram: effectiveRoot,
        subsystems: effectiveSubs,
        libraryPaths: manifest.libraryPaths,
        options: simOptions(),
        emitFromDiagram: true,
      });
      if (invalid.length) {
        r.warnings = [
          ...r.warnings,
          ...invalid.map((v) => ({
            message: `Invalid edge ${v.id}: ${v.reason ?? "rule"}`,
          })),
        ];
      }
      setReport(r);
      setDiagramRev((x) => x + 1);
      setStatus(r.success ? "Validate OK" : "Validate failed");
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
    setBusy(true);
    setTab("results");
    setProgress(null);
    const t0 = performance.now();
    try {
      if (rootDir) await doSave();
      const opts = simOptions();
      const resp = await simulateModel({
        rootDir: rootDir ?? undefined,
        modelName: manifest.modelName,
        diagram: viewId === "root" ? diagram : rootDiagram,
        subsystems: viewId === "root" ? subsystems : { ...subsystems, [viewId]: diagram },
        libraryPaths: manifest.libraryPaths,
        options: opts,
        emitFromDiagram: true,
      });
      if (resp.runId) setRunId(resp.runId);
      if (!resp.success || !resp.result) {
        setReport(
          resp.report ?? {
            success: false,
            stage: "simulate",
            errors: [{ message: resp.cancelled ? "simulation cancelled" : "simulate failed" }],
            warnings: [],
          },
        );
        setTab("messages");
        setStatus(resp.cancelled ? "Cancelled" : "Run failed");
        return;
      }
      const wallMs = Math.round(performance.now() - t0);
      setResult(resp.result);
      setReport({ success: true, stage: "simulate", errors: [], warnings: [] });
      const record: RunRecord = {
        runId: resp.runId ?? `run_${Date.now()}`,
        createdAt: new Date().toISOString(),
        label: `Run ${runs.length + 1}`,
        success: true,
        options: opts,
        result: resp.result,
        wallMs,
      };
      setRuns((prev) => [record, ...prev].slice(0, 20));
      const hints = new Set<string>();
      for (const n of effectiveRoot.nodes) {
        for (const h of n.plotHints) hints.add(`${n.instanceName}.${h}`);
      }
      const keys = Object.keys(resp.result.series);
      const picked = keys.filter((k) =>
        [...hints].some((h) => k === h || k.endsWith(`.${h.split(".").pop()}`)),
      );
      setSelectedVars(picked.length ? picked.slice(0, 4) : keys.slice(0, 3));
      if (!xyX && keys[0]) setXyX(keys[0]);
      if (!xyY && keys[1]) setXyY(keys[1]);
      setStatus(`Run OK (${wallMs}ms)`);
    } catch (e) {
      setReport({
        success: false,
        stage: "simulate",
        errors: [{ message: String(e) }],
        warnings: [],
      });
      setTab("messages");
      setStatus("Run error");
    } finally {
      setBusy(false);
      setRunId(null);
    }
  };

  const doEmit = async (packageFmu: boolean) => {
    const outDir = await pickExportDirectory();
    if (!outDir) return;
    setBusy(true);
    setTab("messages");
    try {
      if (rootDir) await doSave();
      const resp = await emitArtifacts({
        rootDir: rootDir ?? undefined,
        modelName: manifest.modelName,
        diagram: viewId === "root" ? diagram : rootDiagram,
        subsystems: viewId === "root" ? subsystems : { ...subsystems, [viewId]: diagram },
        libraryPaths: manifest.libraryPaths,
        options: simOptions(),
        outDir,
        packageFmu,
        emitFromDiagram: true,
      });
      if (!resp.success) {
        setReport(
          resp.report ?? {
            success: false,
            stage: "emit",
            errors: [{ message: "emit failed" }],
            warnings: [],
          },
        );
        setStatus("Emit failed");
        return;
      }
      setReport({
        success: true,
        stage: "emit",
        errors: [],
        warnings: resp.files.map((f) => ({ message: f })),
      });
      setStatus(
        packageFmu
          ? `FMU: ${resp.fmuPath ?? resp.outDir}`
          : `C emit: ${resp.outDir} (${resp.files.length} files)`,
      );
    } catch (e) {
      setReport({
        success: false,
        stage: "emit",
        errors: [{ message: String(e) }],
        warnings: [],
      });
      setStatus("Emit error");
    } finally {
      setBusy(false);
    }
  };

  const doStop = async () => {
    if (!runId) return;
    const ok = await cancelSimulation(runId);
    setStatus(ok ? "Cancel requested" : "No active run");
  };

  const doCopy = () => {
    clipboard.current = copySelection(diagram, selectedIds);
    if (clipboard.current) setStatus(`Copied ${clipboard.current.nodes.length} block(s)`);
  };

  const doPaste = () => {
    if (!clipboard.current) return;
    replaceDiagram(pasteClipboard(diagram, clipboard.current));
    setStatus("Pasted");
  };

  const doAlign = (mode: "left" | "top" | "hcenter" | "vcenter") => {
    replaceDiagram(alignSelection(diagram, selectedIds, mode));
  };

  const doCreateSubsystem = () => {
    const res = createSubsystemFromSelection(diagram, selectedIds);
    if (!res) {
      setStatus("Select blocks first");
      return;
    }
    setSubsystems((prev) => ({ ...prev, [res.child.diagramId]: res.child }));
    replaceDiagram(res.parent);
    setSelectedIds([res.subsystemNodeId]);
    setStatus(`Created ${res.child.modelName}`);
  };

  const drillInto = (nodeId: string) => {
    const n = diagram.nodes.find((x) => x.id === nodeId);
    if (!n || n.nodeKind !== "subsystem" || !n.subsystemRef) return;
    const child = subsystems[n.subsystemRef];
    if (!child) {
      setStatus("Missing subsystem document");
      return;
    }
    persistView(diagram, viewId);
    setViewStack((s) => [...s, n.subsystemRef!]);
    replace(child);
    setDiagramRev((x) => x + 1);
    setSelectedIds([]);
    setStatus(`Drilled into ${n.instanceName}`);
  };

  const navigateTo = (index: number) => {
    if (index < 0 || index >= viewStack.length) return;
    persistView(diagram, viewId);
    const nextStack = viewStack.slice(0, index + 1);
    const targetId = nextStack[nextStack.length - 1];
    const target = targetId === "root" ? rootDiagram : subsystems[targetId];
    if (!target) return;
    setViewStack(nextStack);
    replace(target);
    setDiagramRev((x) => x + 1);
    setSelectedIds([]);
  };

  const onMessageClick = (message: string) => {
    const id = extractInstanceHint(message, diagram);
    if (id) {
      setSelectedIds([id]);
      setFocusNodeId(id);
      setStatus(`Focused ${id}`);
    }
  };

  useEffect(() => {
    const onKey = (ev: KeyboardEvent) => {
      if (ev.ctrlKey && ev.key.toLowerCase() === "z") {
        ev.preventDefault();
        doUndo();
      }
      if (ev.ctrlKey && ev.key.toLowerCase() === "y") {
        ev.preventDefault();
        doRedo();
      }
      if (ev.ctrlKey && ev.key.toLowerCase() === "c") doCopy();
      if (ev.ctrlKey && ev.key.toLowerCase() === "v") doPaste();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const frac = progress?.fraction ?? null;
  const crumbLabels = viewStack.map((id) => {
    if (id === "root") return manifest.modelName;
    return subsystems[id]?.modelName ?? id;
  });

  return (
    <div className="studio-shell">
      <div className="menu-bar">
        <button type="button" onClick={doNew}>File: New</button>
        <button type="button" onClick={doOpen}>Open</button>
        <button type="button" onClick={doSave}>Save</button>
        <button type="button" onClick={doUndo}>Undo</button>
        <button type="button" onClick={doRedo}>Redo</button>
        <button type="button" onClick={doCopy}>Copy</button>
        <button type="button" onClick={doPaste}>Paste</button>
        <button type="button" onClick={doCreateSubsystem} disabled={selectedIds.length < 1}>
          Create Subsystem
        </button>
        <button
          type="button"
          disabled={!result}
          onClick={() => result && exportResultsCsv(result, `${manifest.modelName}.csv`)}
        >
          Export CSV
        </button>
        <button
          type="button"
          disabled={!result}
          onClick={() => result && exportResultsJson(result, `${manifest.modelName}.json`)}
        >
          Export JSON
        </button>
        <button type="button" disabled={busy} onClick={() => doEmit(false)}>
          Emit C
        </button>
        <button type="button" disabled={busy} onClick={() => doEmit(true)}>
          Emit FMU
        </button>
        <button type="button" disabled={busy} onClick={doImportFmu}>
          Import FMU
        </button>
        <button type="button" disabled title="True pause requires solver hold (Blocked)">
          Pause
        </button>
        <label>
          <input
            type="checkbox"
            checked={showDashboard}
            onChange={(ev) => setShowDashboard(ev.target.checked)}
          />{" "}
          Dashboard
        </label>
        <span className="status-pill">
          {rootDir ?? "(unsaved)"}
          {dirty ? " *" : ""}
        </span>
      </div>
      <div className="breadcrumb">
        {crumbLabels.map((label, i) => (
          <button key={`${viewStack[i]}_${i}`} type="button" onClick={() => navigateTo(i)}>
            {label}
            {i < crumbLabels.length - 1 ? " / " : ""}
          </button>
        ))}
      </div>
      <div className="tool-bar">
        <button type="button" onClick={doSave}>Save</button>
        <button type="button" onClick={doUpdate} disabled={busy}>Update</button>
        <button type="button" className="primary" onClick={doRun} disabled={busy}>Run</button>
        <button type="button" onClick={doStop} disabled={!busy || !runId}>Stop</button>
        <label>
          <input
            type="checkbox"
            checked={snapEnabled}
            onChange={(ev) => setSnapEnabled(ev.target.checked)}
          />{" "}
          Snap
        </label>
        <button type="button" onClick={() => doAlign("left")} disabled={selectedIds.length < 2}>
          Align L
        </button>
        <button type="button" onClick={() => doAlign("top")} disabled={selectedIds.length < 2}>
          Align T
        </button>
        <button type="button" onClick={() => doAlign("hcenter")} disabled={selectedIds.length < 2}>
          Align H
        </button>
        <button type="button" onClick={() => doAlign("vcenter")} disabled={selectedIds.length < 2}>
          Align V
        </button>
        <label>
          Solver
          <select
            value={manifest.simulationDefaults.solver}
            onChange={(ev) =>
              setManifest({
                ...manifest,
                simulationDefaults: { ...manifest.simulationDefaults, solver: ev.target.value },
              })
            }
          >
            <option value="rk45">rk45</option>
            <option value="rk4">rk4</option>
            <option value="implicit">implicit</option>
          </select>
        </label>
        <label>
          stopTime
          <input
            type="number"
            step="0.1"
            value={manifest.simulationDefaults.stopTime}
            onChange={(ev) =>
              setManifest({
                ...manifest,
                simulationDefaults: {
                  ...manifest.simulationDefaults,
                  stopTime: Number(ev.target.value),
                },
              })
            }
          />
        </label>
        <label>
          intervals
          <input
            type="number"
            value={manifest.simulationDefaults.numberOfIntervals}
            onChange={(ev) =>
              setManifest({
                ...manifest,
                simulationDefaults: {
                  ...manifest.simulationDefaults,
                  numberOfIntervals: Number(ev.target.value),
                },
              })
            }
          />
        </label>
        <span className="status-pill">{status}</span>
      </div>
      {busy ? (
        <div className="progress-track">
          <div className="progress-fill" style={{ width: `${Math.round((frac ?? 0) * 100)}%` }} />
          <span className="progress-label">{progress?.message ?? "Running..."}</span>
        </div>
      ) : (
        <div className="progress-track progress-idle" />
      )}
      <div className="main-row">
        <div className="left-column">
          <LibraryPanel
            catalog={catalog}
            filter={filter}
            onFilter={setFilter}
            selectedId={pending?.id ?? null}
            onSelect={setPending}
          />
          <OutlinePanel
            nodes={diagram.nodes}
            selectedIds={selectedIds}
            onSelect={(id) => {
              setSelectedIds([id]);
              setFocusNodeId(id);
            }}
          />
        </div>
        <StudioCanvas
          diagram={diagram}
          diagramRev={diagramRev}
          selectedIds={selectedIds}
          snapEnabled={snapEnabled}
          onDiagramChange={updateDiagram}
          onSelectionChange={setSelectedIds}
          pendingCatalog={pending}
          onConsumePendingCatalog={() => setPending(null)}
          focusNodeId={focusNodeId}
          onNodeDoubleClick={drillInto}
        />
        <div className="right-column">
          <PropertiesPanel
            node={selectedNode}
            catalog={catalog}
            onChange={onNodePropChange}
          />
          {showDashboard ? (
            <DashboardPanel
              nodes={diagram.nodes}
              onChangeParam={(nodeId, paramName, value) => {
                replaceDiagram({
                  ...diagram,
                  nodes: diagram.nodes.map((n) =>
                    n.id !== nodeId
                      ? n
                      : {
                          ...n,
                          params: n.params.map((p) =>
                            p.name === paramName ? { ...p, value } : p,
                          ),
                        },
                  ),
                });
              }}
            />
          ) : null}
        </div>
      </div>
      <ResultsDock
        tab={tab}
        onTab={setTab}
        result={result}
        selectedVars={selectedVars}
        onToggleVar={(name) =>
          setSelectedVars((prev) =>
            prev.includes(name) ? prev.filter((x) => x !== name) : [...prev, name],
          )
        }
        report={report}
        busy={busy}
        progressText={progress?.message ?? null}
        onMessageClick={onMessageClick}
        runs={runs}
        overlayRunIds={overlayRunIds}
        onToggleOverlayRun={(id) =>
          setOverlayRunIds((prev) =>
            prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id],
          )
        }
        xyX={xyX}
        xyY={xyY}
        onXyX={setXyX}
        onXyY={setXyY}
      />
    </div>
  );
}
