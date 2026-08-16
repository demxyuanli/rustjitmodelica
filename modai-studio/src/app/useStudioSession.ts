import { useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { t, tf } from "../i18n";
import { confirmDiscardUnsaved } from "../shell/recentProjects";
import {
  clearCrashDraft,
  clearSessionHeartbeat,
  touchSessionHeartbeat,
  writeCrashDraft,
} from "../project/crashDraft";
import { exportResultsCsv, exportResultsJson } from "../results/exportResults";
import { exportDiagramSvg } from "../util/exportDiagramSvg";
import { exportDiagramPng } from "../util/exportDiagramPng";
import type { MenuBarActions } from "../shell/MenuBar";
import type { WelcomeIntent, WelcomeTab } from "../shell/WelcomeOverlay";
import { useStudioEdit } from "./useStudioEdit";
import { emptySession, useStudioProject } from "./useStudioProject";
import { useStudioSim, type ResultsTab } from "./useStudioSim";
import {
  displayEndValues,
  seriesKeysForEdge,
  seriesKeysForSinkNode,
} from "./plotHints";

export type { ResultsTab };

export function useStudioSession() {
  const [status, setStatus] = useState(() => t("ready"));
  const [findOpen, setFindOpen] = useState(false);
  const [welcomeOpen, setWelcomeOpen] = useState(true);
  const [welcomeTab, setWelcomeTab] = useState<WelcomeTab>("examples");
  const [welcomeIntent, setWelcomeIntent] = useState<WelcomeIntent>("new");
  const [welcomeSeq, setWelcomeSeq] = useState(0);

  const openWelcome = (tab: WelcomeTab, intent: WelcomeIntent = "new") => {
    setWelcomeIntent(intent);
    setWelcomeTab(tab);
    setWelcomeSeq((n) => n + 1);
    setWelcomeOpen(true);
  };
  const boot = emptySession("Plant");
  const edit = useStudioEdit(boot.diagram, setStatus);

  const doSaveRef = useRef<() => Promise<void>>(async () => {});
  const simBridgeRef = useRef({
    setBusy: (_busy: boolean) => {},
    setReport: (_report: import("../api/types").StudioReport | null) => {},
    setTab: (_tab: ResultsTab) => {},
    resetProjectState: (_p: import("../api/types").OpenProjectResult) => {},
    validateOpenedExample: async (_p: import("../api/types").OpenProjectResult) => {},
  });

  const project = useStudioProject({
    setStatus,
    dirty: edit.dirty,
    setDirty: edit.setDirty,
    getSaveSnapshot: edit.getSaveSnapshot,
    syncAfterSave: edit.syncAfterSave,
    applyLoadedProject: edit.applyLoadedProject,
    onProjectLoaded: (p) => simBridgeRef.current.resetProjectState(p),
    setBusy: (busy) => simBridgeRef.current.setBusy(busy),
    setReport: (report) => simBridgeRef.current.setReport(report),
    setTab: (tab) => simBridgeRef.current.setTab(tab),
    onExampleOpened: (p) => simBridgeRef.current.validateOpenedExample(p),
    onNeedProjectFolder: () => openWelcome("new", "saveAs"),
  });

  const sim = useStudioSim({
    setStatus,
    rootDir: project.rootDir,
    manifest: project.manifest,
    dirty: edit.dirty,
    doSave: () => doSaveRef.current(),
    getSaveSnapshot: edit.getSaveSnapshot,
    effectiveRoot: edit.effectiveRoot,
    diagram: edit.diagram,
    bumpDiagramRev: edit.bumpDiagramRev,
    setLoggedVariables: (vars) => {
      project.setManifest({
        ...project.manifest,
        simulationDefaults: {
          ...project.manifest.simulationDefaults,
          loggedVariables: vars,
        },
      });
    },
  });

  doSaveRef.current = project.doSave;
  const dirtyRef = useRef(edit.dirty);
  dirtyRef.current = edit.dirty;
  const rootDirRef = useRef(project.rootDir);
  rootDirRef.current = project.rootDir;

  const requestClose = () => {
    if (!confirmDiscardUnsaved(dirtyRef.current)) return false;
    clearCrashDraft();
    clearSessionHeartbeat();
    return true;
  };

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void getCurrentWindow()
      .onCloseRequested(async (event) => {
        event.preventDefault();
        if (!confirmDiscardUnsaved(dirtyRef.current)) return;
        clearCrashDraft();
        clearSessionHeartbeat();
        try {
          await getCurrentWindow().destroy();
        } catch {
          /* browser preview */
        }
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {
        /* browser preview */
      });
    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    touchSessionHeartbeat();
    const id = window.setInterval(touchSessionHeartbeat, 4000);
    return () => window.clearInterval(id);
  }, []);

  useEffect(() => {
    if (!edit.dirty) return;
    const id = window.setTimeout(() => {
      const snap = edit.getSaveSnapshot();
      writeCrashDraft({
        rootDir: project.rootDir,
        modelName: project.manifest.modelName,
        savedAt: Date.now(),
        manifest: project.manifest,
        diagram: snap.rootDiagram,
        subsystems: snap.subsystems,
      });
    }, 1500);
    return () => window.clearTimeout(id);
  }, [
    edit.dirty,
    edit.diagram,
    edit.subsystems,
    project.rootDir,
    project.manifest,
    edit.getSaveSnapshot,
  ]);

  useEffect(() => {
    const id = window.setInterval(() => {
      if (!dirtyRef.current || !rootDirRef.current) return;
      void doSaveRef.current()
        .then(() => setStatus(t("statusAutosaved")))
        .catch((e: unknown) => setStatus(tf("errGeneric", { error: String(e) })));
    }, 30_000);
    return () => window.clearInterval(id);
  }, [setStatus]);

  simBridgeRef.current = {
    setBusy: sim.setBusy,
    setReport: sim.setReport,
    setTab: sim.setTab,
    resetProjectState: sim.resetProjectState,
    validateOpenedExample: sim.validateOpenedExample,
  };

  // F51: selecting an edge quick-plots its signal series.
  useEffect(() => {
    const edge = edit.selectedEdge;
    if (!edge || !sim.result || edit.selectedIds.length > 0) return;
    const keys = seriesKeysForEdge(
      edit.diagram,
      edge,
      Object.keys(sim.result.series),
    );
    if (keys.length) sim.selectVars(keys);
  }, [edit.selectedEdge, edit.selectedIds.length, edit.diagram, sim.result]);

  // F50: selecting a Scope/Display node binds Results series.
  useEffect(() => {
    if (edit.selectedIds.length !== 1 || !sim.result) return;
    const n = edit.diagram.nodes.find((x) => x.id === edit.selectedIds[0]);
    if (!n) return;
    const cid = (n.catalogId ?? "").toLowerCase();
    if (!cid.includes("sinks.scope") && !cid.includes("sinks.display")) return;
    const keys = seriesKeysForSinkNode(n, Object.keys(sim.result.series));
    if (keys.length) sim.selectVars(keys);
  }, [edit.selectedIds, edit.diagram, sim.result]);

  const displayValues = useMemo(() => {
    if (!sim.result) return {} as Record<string, string>;
    return displayEndValues(edit.diagram, sim.result.series);
  }, [sim.result, edit.diagram]);

  useEffect(() => {
    const onKey = (ev: KeyboardEvent) => {
      const target = ev.target as HTMLElement | null;
      const typing =
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.tagName === "SELECT" ||
          target.isContentEditable);
      if (ev.ctrlKey && ev.key.toLowerCase() === "s") {
        ev.preventDefault();
        void project.doSave();
        return;
      }
      if (ev.ctrlKey && ev.key.toLowerCase() === "o") {
        ev.preventDefault();
        void project.doOpen();
        return;
      }
      if (ev.ctrlKey && ev.key.toLowerCase() === "n") {
        ev.preventDefault();
        setWelcomeIntent("new");
        setWelcomeTab("new");
        setWelcomeSeq((n) => n + 1);
        setWelcomeOpen(true);
        return;
      }
      if (ev.ctrlKey && ev.key === "Enter") {
        ev.preventDefault();
        if (!sim.busy) void sim.doRun();
        return;
      }
      if (ev.ctrlKey && (ev.key === "." || ev.code === "Period")) {
        ev.preventDefault();
        if (sim.busy && sim.runId) void sim.doStop();
        return;
      }
      if (ev.ctrlKey && ev.altKey && ev.key.toLowerCase() === "b") {
        ev.preventDefault();
        project.patchLayout((layout) => ({ showSecondary: !layout.showSecondary }));
        return;
      }
      if (ev.ctrlKey && ev.key.toLowerCase() === "b") {
        ev.preventDefault();
        project.patchLayout((layout) => ({ showSidebar: !layout.showSidebar }));
        return;
      }
      if (ev.ctrlKey && ev.key.toLowerCase() === "j") {
        ev.preventDefault();
        project.patchLayout((layout) => ({ showPanel: !layout.showPanel }));
        return;
      }
      if (ev.key === "Escape" && project.settingsOpen) {
        ev.preventDefault();
        project.setSettingsOpen(false);
        return;
      }
      if (ev.key === "Escape" && project.configOpen) {
        ev.preventDefault();
        project.setConfigOpen(false);
        return;
      }
      if (ev.key === "Escape" && findOpen) {
        ev.preventDefault();
        setFindOpen(false);
        return;
      }
      if (typing) return;
      if (ev.ctrlKey && ev.key.toLowerCase() === "f") {
        ev.preventDefault();
        setFindOpen(true);
        return;
      }
      if (ev.ctrlKey && ev.key.toLowerCase() === "r") {
        ev.preventDefault();
        edit.doRotateSelection();
        return;
      }
      if (ev.key === "?" || (ev.shiftKey && ev.key === "/")) {
        ev.preventDefault();
        project.setSettingsTab("shortcuts");
        project.setSettingsOpen(true);
        return;
      }
      if (ev.code === "Space" || ev.key === " ") {
        if (sim.busy && sim.runId) {
          ev.preventDefault();
          if (sim.paused) void sim.doResume();
          else void sim.doPause();
        }
        return;
      }
      if ((ev.key === "." || ev.code === "Period") && !ev.ctrlKey && !ev.altKey && !ev.metaKey) {
        if (sim.busy && sim.runId) {
          ev.preventDefault();
          void sim.doStep();
        }
        return;
      }
      if (ev.ctrlKey && ev.key.toLowerCase() === "z") {
        ev.preventDefault();
        edit.doUndo();
      }
      if (ev.ctrlKey && ev.key.toLowerCase() === "y") {
        ev.preventDefault();
        edit.doRedo();
      }
      if (ev.ctrlKey && ev.key.toLowerCase() === "c") edit.doCopy();
      if (ev.ctrlKey && ev.key.toLowerCase() === "v") edit.doPaste();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const crumbLabels = edit.viewStack.map((id) => {
    if (id === "root") return project.manifest.modelName;
    return edit.subsystems[id]?.modelName ?? id;
  });

  const menuActions: MenuBarActions = {
    onNew: () => openWelcome("new"),
    onOpen: () => void project.doOpen(),
    onOpenExample: (id) => void project.doOpenExample(id),
    onSave: () => void project.doSave(),
    onUndo: edit.doUndo,
    onRedo: edit.doRedo,
    onCopy: edit.doCopy,
    onPaste: edit.doPaste,
    onSubsystem: edit.doCreateSubsystem,
    onAddVariant: edit.doAddVariant,
    onExportCsv: () => sim.result && exportResultsCsv(sim.result, `${project.manifest.modelName}.csv`),
    onExportJson: () => sim.result && exportResultsJson(sim.result, `${project.manifest.modelName}.json`),
    onExportSvg: () => exportDiagramSvg(edit.diagram, `${project.manifest.modelName}_diagram.svg`),
    onExportPng: () => {
      void exportDiagramPng(edit.diagram, `${project.manifest.modelName}_diagram.png`).catch((e) =>
        setStatus(tf("errGeneric", { error: String(e) })),
      );
    },
    onEmitCoder: () => sim.openExportWizard("coder"),
    onEmitFmu: () => sim.openExportWizard("fmu-cs"),
    onImportFmu: () => void project.doImportFmu(),
    onWrapMo: () => void project.doWrapMo(),
    onImportPack: () => void project.doImportPack(),
    onExportPack: () => void project.doExportPack(),
    onPause: () => void sim.doPause(),
    onResume: () => void sim.doResume(),
    onStep: () => void sim.doStep(),
    onConfig: () => project.setConfigOpen(true),
    onSettings: () => project.openSettings("general"),
    onWelcome: () => openWelcome("examples"),
    canExport: !!sim.result,
    busy: sim.busy,
    runId: sim.runId,
    paused: sim.paused,
    canSubsystem: edit.selectedIds.length >= 1,
    canAddVariant: (() => {
      const id = edit.selectedIds[0];
      if (!id || edit.selectedIds.length !== 1) return false;
      const n = edit.diagram.nodes.find((x) => x.id === id);
      return !!n && n.nodeKind === "subsystem";
    })(),
    projectLabel: project.projectLabel,
  };

  return {
    settings: project.settings,
    updateSettings: project.updateSettings,
    patchLayout: project.patchLayout,
    layout: project.layout,
    setActivity: project.setActivity,
    openSettings: project.openSettings,
    settingsOpen: project.settingsOpen,
    setSettingsOpen: project.setSettingsOpen,
    settingsTab: project.settingsTab,
    setSettingsTab: project.setSettingsTab,
    configOpen: project.configOpen,
    setConfigOpen: project.setConfigOpen,
    requestClose,
    menuActions,
    manifest: project.manifest,
    setManifest: project.setManifest,
    dirty: edit.dirty,
    catalog: project.catalog,
    filter: project.filter,
    setFilter: project.setFilter,
    pending: project.pending,
    setPending: project.setPending,
    diagram: edit.diagram,
    diagramRev: edit.diagramRev,
    selectedIds: edit.selectedIds,
    selectedEdgeIds: edit.selectedEdgeIds,
    setSelectedIds: edit.setSelectedIds,
    setSelection: edit.setSelection,
    selectOutlineNode: edit.selectOutlineNode,
    selectedNode: edit.selectedNode,
    selectedEdge: edit.selectedEdge,
    onNodePropChange: edit.onNodePropChange,
    onEdgePropChange: edit.onEdgePropChange,
    onApplyNodes: edit.onApplyNodes,
    doAddVariant: edit.doAddVariant,
    doSetActiveVariant: edit.doSetActiveVariant,
    doRenameVariant: edit.doRenameVariant,
    doRemoveVariant: edit.doRemoveVariant,
    onDashboardParamChange: edit.onDashboardParamChange,
    showDashboard: edit.showDashboard,
    setShowDashboard: edit.setShowDashboard,
    snapEnabled: edit.snapEnabled,
    setSnapEnabled: edit.setSnapEnabled,
    fastRestart: sim.fastRestart,
    setFastRestart: sim.setFastRestart,
    fullJitValidate: sim.fullJitValidate,
    setFullJitValidate: sim.setFullJitValidate,
    focusNodeId: edit.focusNodeId,
    updateDiagram: edit.updateDiagram,
    drillInto: edit.drillInto,
    navigateTo: edit.navigateTo,
    viewStack: edit.viewStack,
    crumbLabels,
    projectLabel: project.projectLabel,
    status,
    busy: sim.busy,
    runId: sim.runId,
    progressLabel: sim.progressLabel,
    progressFraction: sim.progressFraction,
    doSave: project.doSave,
    doSaveAs: project.doSaveAs,
    doNew: project.doNew,
    doOpen: project.doOpen,
    doOpenRecent: project.doOpenRecent,
    doOpenExample: project.doOpenExample,
    recentProjects: project.recentProjects,
    rootDir: project.rootDir,
    doUpdate: sim.doUpdate,
    doRun: sim.doRun,
    doStop: sim.doStop,
    bgValidate: sim.bgValidate,
    cancelBgValidate: sim.cancelBgValidate,
    setStatus,
    doCreateSubsystem: edit.doCreateSubsystem,
    doDeleteSelection: edit.doDeleteSelection,
    doPlotSelection: () => {
      const edge = edit.selectedEdge;
      if (!edge || !sim.result) return;
      const keys = seriesKeysForEdge(edit.diagram, edge, Object.keys(sim.result.series));
      if (keys.length) sim.selectVars(keys);
      sim.setTab("results");
    },
    doAlign: edit.doAlign,
    doReroute: edit.doReroute,
    doRotateSelection: edit.doRotateSelection,
    welcomeOpen,
    welcomeTab,
    welcomeIntent,
    welcomeSeq,
    setWelcomeOpen,
    findOpen,
    setFindOpen,
    tab: sim.tab,
    setTab: sim.setTab,
    result: sim.result,
    selectedVars: sim.selectedVars,
    toggleSelectedVar: sim.toggleSelectedVar,
    selectVars: sim.selectVars,
    displayValues,
    report: sim.report,
    onMessageClick: edit.onMessageClick,
    runs: sim.runs,
    overlayRunIds: sim.overlayRunIds,
    toggleOverlayRun: sim.toggleOverlayRun,
    xyX: sim.xyX,
    xyY: sim.xyY,
    setXyX: sim.setXyX,
    setXyY: sim.setXyY,
    lastProfile: sim.lastProfile,
    profileRunId: sim.profileRunId,
    profileWallMs: sim.profileWallMs,
    selectProfileRun: sim.selectProfileRun,
    exportWizardOpen: sim.exportWizardOpen,
    exportPreset: sim.exportPreset,
    openExportWizard: sim.openExportWizard,
    closeExportWizard: sim.closeExportWizard,
    doEmitWithOptions: sim.doEmitWithOptions,
    emitHistory: sim.emitHistory,
    effectiveRoot: edit.effectiveRoot,
  };
}

export type StudioSession = ReturnType<typeof useStudioSession>;
