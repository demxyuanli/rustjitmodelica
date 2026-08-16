import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  catalogList,
  importFmu,
  openExample,
  pickDirectory,
  pickCatalogPackDirectory,
  pickExportDirectory,
  pickFmuFile,
  pickMoFile,
  projectCreate,
  projectOpen,
  projectSave,
  startupProjectPath,
  wrapMo,
  importCatalogPack,
  exportCatalogPack,
} from "../api/studio";
import type {
  CatalogEntry,
  DiagramDocument,
  OpenProjectResult,
  SimulationDefaults,
  StudioProjectManifest,
} from "../api/types";
import { setI18nLocale, t, tf } from "../i18n";
import {
  applySettingsToDom,
  clampLayout,
  loadSettings,
  saveSettings,
  type ActivityId,
  type StudioSettings,
} from "../settings/studioSettings";
import type { SettingsTab } from "../shell/shortcuts";
import {
  confirmDiscardUnsaved,
  forgetRecentProject,
  loadRecentProjects,
  rememberRecentProject,
  type RecentProject,
} from "../shell/recentProjects";
import type { StudioEditSaveSnapshot } from "./useStudioEdit";
import {
  clearCrashDraft,
  confirmRestoreDraft,
  draftHasWork,
  isSessionStale,
  readCrashDraft,
  type CrashDraft,
} from "../project/crashDraft";

export type NewProjectInit = {
  modelName: string;
  parentDir: string;
  templateId?: string | null;
  simulationDefaults: SimulationDefaults;
};

const LAST_NEW_PARENT_KEY = "modai-studio.last-new-parent";

export function sanitizeModelName(raw: string): string {
  const s = raw.trim().replace(/[^A-Za-z0-9_]/g, "_");
  if (!s || !/^[A-Za-z_]/.test(s)) return "Plant";
  return s;
}

export function joinProjectDir(parent: string, name: string): string {
  const p = parent.trim().replace(/[/\\]+$/, "");
  if (!p) return "";
  const sep = /\\/.test(p) && !p.startsWith("/") ? "\\" : "/";
  return `${p}${sep}${name}`;
}

export function loadLastNewParent(): string {
  try {
    return localStorage.getItem(LAST_NEW_PARENT_KEY) ?? "";
  } catch {
    return "";
  }
}

function rememberLastNewParent(dir: string) {
  try {
    localStorage.setItem(LAST_NEW_PARENT_KEY, dir);
  } catch {
    /* ignore quota / private mode */
  }
}

export function emptySession(modelName: string): {
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
      paramDictionary: [],
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

type StudioProjectDeps = {
  setStatus: (status: string) => void;
  dirty: boolean;
  setDirty: (dirty: boolean) => void;
  getSaveSnapshot: () => StudioEditSaveSnapshot;
  syncAfterSave: (root: DiagramDocument, subs: Record<string, DiagramDocument>) => void;
  applyLoadedProject: (p: OpenProjectResult) => void;
  onProjectLoaded: (p: OpenProjectResult) => void;
  setBusy: (busy: boolean) => void;
  setReport: (report: import("../api/types").StudioReport | null) => void;
  setTab: (tab: import("./useStudioSim").ResultsTab) => void;
  onExampleOpened: (p: OpenProjectResult) => Promise<void>;
  onNeedProjectFolder: () => void;
};

export function useStudioProject(deps: StudioProjectDeps) {
  const boot = useMemo(() => emptySession("Plant"), []);
  const [rootDir, setRootDir] = useState<string | null>(boot.rootDir);
  const [manifest, setManifest] = useState(boot.manifest);
  const [catalog, setCatalog] = useState<CatalogEntry[]>([]);
  const [filter, setFilter] = useState("");
  const [pending, setPending] = useState<CatalogEntry | null>(null);
  const [settings, setSettings] = useState<StudioSettings>(() => loadSettings());
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsTab, setSettingsTab] = useState<SettingsTab>("general");
  const [configOpen, setConfigOpen] = useState(false);
  const [recentProjects, setRecentProjects] = useState<RecentProject[]>(() =>
    loadRecentProjects(),
  );

  const {
    setStatus,
    dirty,
    setDirty,
    getSaveSnapshot,
    syncAfterSave,
    applyLoadedProject,
    onProjectLoaded,
    setBusy,
    setReport,
    setTab,
    onExampleOpened,
    onNeedProjectFolder,
  } = deps;

  useEffect(() => {
    setI18nLocale(settings.locale);
    applySettingsToDom(settings);
    saveSettings(settings);
  }, [settings]);

  const updateSettings = useCallback((next: StudioSettings) => {
    setI18nLocale(next.locale);
    applySettingsToDom(next);
    setSettings(next);
  }, []);

  const patchLayout = useCallback(
    (
      partial:
        | Partial<StudioSettings["layout"]>
        | ((prev: StudioSettings["layout"]) => Partial<StudioSettings["layout"]>),
    ) => {
      setSettings((prev) => {
        const delta = typeof partial === "function" ? partial(prev.layout) : partial;
        const next = {
          ...prev,
          layout: clampLayout({ ...prev.layout, ...delta }),
        };
        applySettingsToDom(next);
        saveSettings(next);
        return next;
      });
    },
    [],
  );

  useEffect(() => {
    catalogList(rootDir)
      .then(setCatalog)
      .catch((e: unknown) => setStatus(tf("errGeneric", { error: String(e) })));
  }, [rootDir, setStatus]);

  const loadProject = useCallback(
    (p: OpenProjectResult) => {
      setRootDir(p.rootDir);
      setManifest(p.manifest);
      applyLoadedProject(p);
      onProjectLoaded(p);
      if (p.rootDir) {
        setRecentProjects(rememberRecentProject(p.rootDir, p.manifest.modelName));
      }
      setStatus(tf("statusOpened", { path: p.rootDir }));
    },
    [applyLoadedProject, onProjectLoaded, setStatus],
  );

  const restoreDraft = useCallback(
    (draft: CrashDraft) => {
      const opened: OpenProjectResult = {
        rootDir: draft.rootDir ?? "",
        manifest: draft.manifest,
        diagram: draft.diagram,
        modelSource: "",
        subsystems: draft.subsystems ?? {},
      };
      setRootDir(draft.rootDir);
      setManifest(draft.manifest);
      applyLoadedProject(opened);
      onProjectLoaded(opened);
      setDirty(true);
      setStatus(t("statusDraftRestored"));
    },
    [applyLoadedProject, onProjectLoaded, setDirty, setStatus],
  );

  const autoloadDone = useRef(false);
  useEffect(() => {
    if (autoloadDone.current) return;
    autoloadDone.current = true;
    void (async () => {
      try {
        const draft = readCrashDraft();
        if (draft && draftHasWork(draft) && isSessionStale() && confirmRestoreDraft(draft)) {
          restoreDraft(draft);
          return;
        }
        if (draft) clearCrashDraft();
        const path = await startupProjectPath();
        if (!path) return;
        const opened = await projectOpen(path);
        loadProject(opened);
      } catch (e) {
        setStatus(tf("errGeneric", { error: String(e) }));
      }
    })();
  }, [loadProject, restoreDraft, setStatus]);

  const doSave = async () => {
    const { rootDiagram: root, subsystems: subs } = getSaveSnapshot();
    const manifestToSave: StudioProjectManifest = {
      ...manifest,
      ui: { ...manifest.ui, locale: settings.locale },
    };
    if (!rootDir) {
      onNeedProjectFolder();
      return;
    }
    await projectSave(rootDir, manifestToSave, root, subs);
    setManifest(manifestToSave);
    syncAfterSave(root, subs);
    setDirty(false);
    setStatus(t("statusSaved"));
    clearCrashDraft();
  };

  const doSaveAs = async (init: NewProjectInit): Promise<boolean> => {
    const name = sanitizeModelName(init.modelName);
    const dest = joinProjectDir(init.parentDir, name);
    if (!dest) return false;
    const { rootDiagram: root, subsystems: subs } = getSaveSnapshot();
    try {
      rememberLastNewParent(init.parentDir.trim());
      await projectCreate(dest, name);
      const nextManifest: StudioProjectManifest = {
        ...manifest,
        modelName: name,
        simulationDefaults: {
          ...init.simulationDefaults,
          loggedVariables: manifest.simulationDefaults.loggedVariables ?? [],
        },
        ui: { ...manifest.ui, locale: settings.locale },
      };
      const nextDiagram: DiagramDocument = { ...root, modelName: name };
      await projectSave(dest, nextManifest, nextDiagram, subs);
      const opened = await projectOpen(dest);
      loadProject(opened);
      setStatus(t("statusSaved"));
      clearCrashDraft();
      return true;
    } catch (e) {
      setStatus(tf("errProjectCreate", { error: String(e) }));
      return false;
    }
  };

  const doNew = async (init: NewProjectInit): Promise<boolean> => {
    if (!confirmDiscardUnsaved(dirty)) return false;
    const name = sanitizeModelName(init.modelName);
    const dest = joinProjectDir(init.parentDir, name);
    if (!dest) return false;
    try {
      rememberLastNewParent(init.parentDir.trim());
      if (init.templateId) {
        const src = await openExample(init.templateId);
        await projectCreate(dest, name);
        const manifest: StudioProjectManifest = {
          ...src.manifest,
          modelName: name,
          simulationDefaults: init.simulationDefaults,
        };
        const diagram: DiagramDocument = { ...src.diagram, modelName: name };
        await projectSave(dest, manifest, diagram, src.subsystems ?? {});
      } else {
        const created = await projectCreate(dest, name);
        const manifest: StudioProjectManifest = {
          ...created.manifest,
          modelName: name,
          simulationDefaults: init.simulationDefaults,
        };
        await projectSave(dest, manifest, created.diagram, created.subsystems ?? {});
      }
      const opened = await projectOpen(dest);
      loadProject(opened);
      setStatus(tf("statusCreated", { name }));
      clearCrashDraft();
      return true;
    } catch (e) {
      setStatus(tf("errProjectCreate", { error: String(e) }));
      return false;
    }
  };

  const doOpen = async (): Promise<boolean> => {
    if (!confirmDiscardUnsaved(dirty)) return false;
    const dir = await pickDirectory();
    if (!dir) return false;
    const opened = await projectOpen(dir);
    loadProject(opened);
    return true;
  };

  const doOpenRecent = async (path: string): Promise<boolean> => {
    if (!confirmDiscardUnsaved(dirty)) return false;
    try {
      const opened = await projectOpen(path);
      loadProject(opened);
      return true;
    } catch (e) {
      setRecentProjects(forgetRecentProject(path));
      setStatus(tf("errRecentMissing", { path, error: String(e) }));
      return false;
    }
  };

  const doOpenExample = async (id: string): Promise<boolean> => {
    if (!confirmDiscardUnsaved(dirty)) return false;
    try {
      const opened = await openExample(id);
      loadProject(opened);
      setStatus(tf("statusExampleOpened", { name: opened.manifest.modelName }));
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          void onExampleOpened(opened);
        });
      });
      return true;
    } catch (e) {
      setStatus(tf("errGeneric", { error: String(e) }));
      return false;
    }
  };

  const doImportFmu = async () => {
    const path = await pickFmuFile();
    if (!path) return;
    setBusy(true);
    try {
      const res = await importFmu({ fmuPath: path, projectRoot: rootDir });
      if (!res.success || !res.catalogEntry) {
        const msg = res.error ?? t("statusFmuImportFailed");
        setStatus(msg);
        setReport({
          success: false,
          stage: "fmu_import",
          errors: [{ message: msg }],
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
      const binNote = res.hasBinary ? t("statusBinaryOk") : t("statusNoBinary");
      setStatus(tf("statusFmuImported", { id: res.modelIdentifier, note: binNote }));
    } catch (e) {
      setStatus(tf("errGeneric", { error: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  const doWrapMo = async () => {
    const path = await pickMoFile();
    if (!path) return;
    setBusy(true);
    try {
      const res = await wrapMo({ moPath: path, projectRoot: rootDir });
      if (!res.success || !res.catalogEntry) {
        const msg = res.error ?? t("statusWrapMoFailed");
        setStatus(msg);
        setReport({
          success: false,
          stage: "wrap_mo",
          errors: [{ message: msg }],
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
      if (rootDir) {
        const alt = `${rootDir.replace(/[/\\]+$/, "")}/wrapped`;
        setManifest((m) => {
          const paths = m.libraryPaths ?? [];
          if (paths.some((p) => p === alt || p.endsWith("wrapped") || p.endsWith("wrapped\\") || p.endsWith("wrapped/"))) {
            return m;
          }
          return { ...m, libraryPaths: [...paths, alt] };
        });
      }
      setStatus(
        tf("statusWrapMoOk", {
          name: res.modelName,
          ins: res.inputCount,
          outs: res.outputCount,
          params: res.paramCount,
          icon: res.hasIcon ? t("statusWrapMoIconYes") : t("statusWrapMoIconNo"),
        }),
      );
    } catch (e) {
      setStatus(tf("errGeneric", { error: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  const doImportPack = async () => {
    if (!rootDir) {
      setStatus(t("statusNeedProject"));
      return;
    }
    const packDir = await pickCatalogPackDirectory();
    if (!packDir) return;
    setBusy(true);
    try {
      const res = await importCatalogPack({ packDir, projectRoot: rootDir });
      if (!res.success) {
        setStatus(res.error ?? t("statusImportPackFailed"));
        return;
      }
      setCatalog((prev) => {
        const ids = new Set(res.entries.map((e) => e.id));
        return [...prev.filter((c) => !ids.has(c.id)), ...res.entries];
      });
      if (res.libraryPath) {
        setManifest((m) => {
          const paths = m.libraryPaths ?? [];
          if (paths.includes(res.libraryPath)) return m;
          return { ...m, libraryPaths: [...paths, res.libraryPath] };
        });
      }
      if (res.entries[0]) setPending(res.entries[0]);
      setStatus(tf("statusImportPackOk", { name: res.packName, n: res.copied }));
    } catch (e) {
      setStatus(tf("errGeneric", { error: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  const doExportPack = async () => {
    if (!rootDir) {
      setStatus(t("statusNeedProject"));
      return;
    }
    const outDir = await pickExportDirectory();
    if (!outDir) return;
    setBusy(true);
    try {
      const assets = catalog
        .flatMap((c) => (c.tags ?? []).filter((t) => t.startsWith("asset:")))
        .map((t) => t.slice("asset:".length));
      const res = await exportCatalogPack({
        projectRoot: rootDir,
        relativeAssets: assets,
        outDir,
        packName: "UserLib",
      });
      if (!res.success) {
        setStatus(res.error ?? t("statusExportPackFailed"));
        return;
      }
      setStatus(tf("statusExportPackOk", { dir: res.outDir, n: res.copied }));
    } catch (e) {
      setStatus(tf("errGeneric", { error: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  const openSettings = (nextTab: SettingsTab = "general") => {
    setSettingsTab(nextTab);
    setSettingsOpen(true);
  };

  const setActivity = (id: ActivityId) => {
    patchLayout((prev) => {
      if (id === "properties") {
        return { activity: "properties", showSecondary: true, showSidebar: true };
      }
      if (prev.activity === id && prev.showSidebar) {
        return { showSidebar: false };
      }
      return { activity: id, showSidebar: true };
    });
  };

  const projectLabel = `${rootDir ?? t("unsaved")}${dirty ? " *" : ""}`;
  const layout = settings.layout;

  return {
    rootDir,
    manifest,
    setManifest,
    catalog,
    filter,
    setFilter,
    pending,
    setPending,
    settings,
    updateSettings,
    patchLayout,
    layout,
    setActivity,
    openSettings,
    settingsOpen,
    setSettingsOpen,
    settingsTab,
    setSettingsTab,
    configOpen,
    setConfigOpen,
    doSave,
    doSaveAs,
    doNew,
    doOpen,
    doOpenRecent,
    doOpenExample,
    recentProjects,
    doImportFmu,
    doWrapMo,
    doImportPack,
    doExportPack,
    loadProject,
    projectLabel,
  };
}
