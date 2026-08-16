import {
  applyThemeToDom,
  DEFAULT_THEME_ID,
  resolveThemeId,
  type ThemeId,
} from "./themes";

export type { ThemeId };
export type Locale = "zh" | "en";

export type MenuItemId =
  | "welcome"
  | "new"
  | "open"
  | "examples"
  | "save"
  | "undo"
  | "redo"
  | "copy"
  | "paste"
  | "subsystem"
  | "addVariant"
  | "exportCsv"
  | "exportJson"
  | "exportSvg"
  | "exportPng"
  | "emitCoder"
  | "emitFmu"
  | "importFmu"
  | "importIde"
  | "wrapMo"
  | "importPack"
  | "exportPack"
  | "pause"
  | "resume"
  | "step"
  | "config"
  | "settings";

export type ToolItemId =
  | "save"
  | "update"
  | "run"
  | "stop"
  | "pause"
  | "resume"
  | "step"
  | "snap"
  | "alignL"
  | "alignT"
  | "alignH"
  | "alignV"
  | "reroute"
  | "solver"
  | "stopTime"
  | "intervals"
  | "config"
  | "dashboard"
  | "fastRestart"
  | "fullJitValidate";

export type ActivityId = "explorer" | "properties" | "data";

export interface StudioLayout {
  leftWidth: number;
  rightWidth: number;
  libraryPct: number;
  resultsHeight: number;
  activity: ActivityId;
  showSidebar: boolean;
  showPanel: boolean;
  showSecondary: boolean;
}

/** Persisted Export Wizard defaults (F90). */
export interface ExportOptionsPrefs {
  mode: "coder" | "fmu-cs" | "fmu-me";
  coderPackage: boolean;
  modelIdentifier: string;
  guid: string;
  lastOutDir: string;
}

export const DEFAULT_EXPORT_PREFS: ExportOptionsPrefs = {
  mode: "coder",
  coderPackage: true,
  modelIdentifier: "",
  guid: "",
  lastOutDir: "",
};

export interface StudioSettings {
  locale: Locale;
  themeId: ThemeId;
  fontFamily: string;
  fontSizePx: number;
  showTips: boolean;
  iconLabels: boolean;
  density: "comfortable" | "compact";
  /** Show signal names / units on edges. */
  edgeLabelsVisible: boolean;
  /** Vertical offset for edge labels (px). */
  edgeLabelOffsetY: number;
  menuVisible: Record<MenuItemId, boolean>;
  toolVisible: Record<ToolItemId, boolean>;
  layout: StudioLayout;
  /** Last Export Suite options. */
  lastExport: ExportOptionsPrefs;
}

export const MENU_IDS: MenuItemId[] = [
  "welcome",
  "new",
  "open",
  "examples",
  "save",
  "undo",
  "redo",
  "copy",
  "paste",
  "subsystem",
  "addVariant",
  "exportCsv",
  "exportJson",
  "exportSvg",
  "exportPng",
  "emitCoder",
  "emitFmu",
  "importFmu",
  "importIde",
  "wrapMo",
  "importPack",
  "exportPack",
  "pause",
  "resume",
  "step",
  "config",
  "settings",
];

export const TOOL_IDS: ToolItemId[] = [
  "save",
  "update",
  "run",
  "stop",
  "pause",
  "resume",
  "step",
  "snap",
  "alignL",
  "alignT",
  "alignH",
  "alignV",
  "reroute",
  "solver",
  "stopTime",
  "intervals",
  "config",
  "dashboard",
  "fastRestart",
  "fullJitValidate",
];

export const FONT_PRESETS = [
  '"Segoe UI", system-ui, sans-serif',
  '"Cascadia Code", "Consolas", monospace',
  '"IBM Plex Sans", "Segoe UI", sans-serif',
  '"Noto Sans SC", "Microsoft YaHei", sans-serif',
];

export const DEFAULT_LAYOUT: StudioLayout = {
  leftWidth: 240,
  rightWidth: 280,
  libraryPct: 55,
  resultsHeight: 260,
  activity: "explorer",
  showSidebar: true,
  showPanel: true,
  showSecondary: true,
};

function allTrue<T extends string>(ids: T[]): Record<T, boolean> {
  const out = {} as Record<T, boolean>;
  for (const id of ids) out[id] = true;
  return out;
}

export const DEFAULT_SETTINGS: StudioSettings = {
  locale: "zh",
  themeId: DEFAULT_THEME_ID,
  fontFamily: FONT_PRESETS[0]!,
  fontSizePx: 13,
  showTips: true,
  iconLabels: false,
  density: "comfortable",
  edgeLabelsVisible: true,
  edgeLabelOffsetY: -10,
  menuVisible: allTrue(MENU_IDS),
  toolVisible: {
    ...allTrue(TOOL_IDS),
    intervals: false,
    fullJitValidate: false,
  },
  layout: { ...DEFAULT_LAYOUT },
  lastExport: { ...DEFAULT_EXPORT_PREFS },
};

const STORAGE_KEY = "modai-studio.settings.v1";

function clamp(n: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, n));
}

export function clampLayout(layout: Partial<StudioLayout> | undefined): StudioLayout {
  const src = { ...DEFAULT_LAYOUT, ...(layout ?? {}) };
  const activity: ActivityId =
    src.activity === "properties"
      ? "properties"
      : src.activity === "data"
        ? "data"
        : "explorer";
  return {
    leftWidth: clamp(Math.round(src.leftWidth), 160, 520),
    rightWidth: clamp(Math.round(src.rightWidth), 200, 520),
    libraryPct: clamp(Math.round(src.libraryPct), 20, 80),
    resultsHeight: clamp(Math.round(src.resultsHeight), 120, 520),
    activity,
    showSidebar: src.showSidebar !== false,
    showPanel: src.showPanel !== false,
    showSecondary: src.showSecondary !== false,
  };
}

export function loadSettings(): StudioSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return {
        ...DEFAULT_SETTINGS,
        menuVisible: { ...DEFAULT_SETTINGS.menuVisible },
        toolVisible: { ...DEFAULT_SETTINGS.toolVisible },
        layout: { ...DEFAULT_LAYOUT },
        lastExport: { ...DEFAULT_EXPORT_PREFS },
      };
    }
    const parsed = JSON.parse(raw) as Partial<StudioSettings>;
    const lastExport = {
      ...DEFAULT_EXPORT_PREFS,
      ...(parsed.lastExport ?? {}),
    };
    if (lastExport.mode !== "coder" && lastExport.mode !== "fmu-cs" && lastExport.mode !== "fmu-me") {
      lastExport.mode = "coder";
    }
    return {
      ...DEFAULT_SETTINGS,
      ...parsed,
      themeId: resolveThemeId(parsed.themeId),
      edgeLabelsVisible: parsed.edgeLabelsVisible !== false,
      edgeLabelOffsetY:
        typeof parsed.edgeLabelOffsetY === "number"
          ? clamp(Math.round(parsed.edgeLabelOffsetY), -40, 40)
          : DEFAULT_SETTINGS.edgeLabelOffsetY,
      menuVisible: { ...DEFAULT_SETTINGS.menuVisible, ...(parsed.menuVisible ?? {}) },
      toolVisible: { ...DEFAULT_SETTINGS.toolVisible, ...(parsed.toolVisible ?? {}) },
      layout: clampLayout(parsed.layout),
      lastExport,
    };
  } catch {
    return {
      ...DEFAULT_SETTINGS,
      menuVisible: { ...DEFAULT_SETTINGS.menuVisible },
      toolVisible: { ...DEFAULT_SETTINGS.toolVisible },
      layout: { ...DEFAULT_LAYOUT },
      lastExport: { ...DEFAULT_EXPORT_PREFS },
    };
  }
}

export function saveSettings(s: StudioSettings): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
}

export function applySettingsToDom(s: StudioSettings): void {
  const root = document.documentElement;
  applyThemeToDom(resolveThemeId(s.themeId));
  root.style.setProperty("--studio-font", s.fontFamily);
  root.style.setProperty("--studio-font-size", `${s.fontSizePx}px`);
  root.style.setProperty("--studio-left-w", `${s.layout.leftWidth}px`);
  root.style.setProperty("--studio-right-w", `${s.layout.rightWidth}px`);
  root.style.setProperty("--studio-library-pct", `${s.layout.libraryPct}%`);
  root.style.setProperty("--studio-results-h", `${s.layout.showPanel ? s.layout.resultsHeight : 0}px`);
  root.dataset.density = s.density;
  root.dataset.locale = s.locale;
  root.dataset.sidebar = s.layout.showSidebar ? "1" : "0";
  root.dataset.panel = s.layout.showPanel ? "1" : "0";
  root.dataset.secondary = s.layout.showSecondary ? "1" : "0";
}
