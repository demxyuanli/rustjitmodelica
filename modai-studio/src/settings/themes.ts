import { getCurrentWindow } from "@tauri-apps/api/window";

/** Classic Studio theme catalog and CSS variable application. */

export type ThemeId = "dark" | "light" | "classic" | "highContrast";

export interface ThemeTokens {
  bg: string;
  bgElevated: string;
  panel: string;
  panelAlt: string;
  border: string;
  borderSubtle: string;
  text: string;
  muted: string;
  accent: string;
  accentFg: string;
  danger: string;
  ok: string;
  signal: string;
  connector: string;
  inport: string;
  outport: string;
  busbar: string;
  goto: string;
  from: string;
  surfaceInput: string;
  surfaceHover: string;
  surfaceActive: string;
  titlebarBg: string;
  activityBg: string;
  statusbarBg: string;
  statusbarFg: string;
  canvasBg: string;
  canvasGrid: string;
  btnBg: string;
  btnHover: string;
  winCloseHover: string;
  progress: string;
  progressEnd: string;
  overlay: string;
  shadow: string;
  blockSignal: string;
  blockElectrical: string;
  blockMultibody: string;
  blockSubsystem: string;
  blockFmu: string;
  blockSubBorder: string;
  blockFmuBorder: string;
}

export interface ThemeDefinition {
  id: ThemeId;
  labelKey: "themeDark" | "themeLight" | "themeClassic" | "themeHighContrast";
  tokens: ThemeTokens;
}

export const DEFAULT_THEME_ID: ThemeId = "dark";

export const CLASSIC_THEMES: ThemeDefinition[] = [
  {
    id: "dark",
    labelKey: "themeDark",
    tokens: {
      bg: "#1b1f24",
      bgElevated: "#14181e",
      panel: "#23282f",
      panelAlt: "#1a1f26",
      border: "#3a424d",
      borderSubtle: "#2a3038",
      text: "#e8ecf1",
      muted: "#9aa3ad",
      accent: "#3d8bfd",
      accentFg: "#ffffff",
      danger: "#e35d6a",
      ok: "#3dbf7a",
      signal: "#5b9bd5",
      connector: "#c47a3a",
      inport: "#3b82c4",
      outport: "#e07a3d",
      busbar: "#6b7c93",
      goto: "#9b6bb8",
      from: "#3a9a7a",
      surfaceInput: "#15191e",
      surfaceHover: "#2a3038",
      surfaceActive: "#2a3544",
      titlebarBg: "#14181e",
      activityBg: "#181c22",
      statusbarBg: "#0e639c",
      statusbarFg: "#ffffff",
      canvasBg: "#12161b",
      canvasGrid: "#2a3038",
      btnBg: "#2d3440",
      btnHover: "#2a3544",
      winCloseHover: "#e81123",
      progress: "#2a6fdb",
      progressEnd: "#3d8bfd",
      overlay: "rgba(0, 0, 0, 0.55)",
      shadow: "rgba(0, 0, 0, 0.4)",
      blockSignal: "#243041",
      blockElectrical: "#3a2f24",
      blockMultibody: "#24302a",
      blockSubsystem: "#2a3a2a",
      blockFmu: "#2a2a3a",
      blockSubBorder: "#5a9a5a",
      blockFmuBorder: "#7a7ad0",
    },
  },
  {
    id: "light",
    labelKey: "themeLight",
    tokens: {
      bg: "#f3f5f8",
      bgElevated: "#e8ecf1",
      panel: "#ffffff",
      panelAlt: "#eef1f5",
      border: "#c5ced8",
      borderSubtle: "#d7dee6",
      text: "#1c2430",
      muted: "#5b6775",
      accent: "#0b6bcb",
      accentFg: "#ffffff",
      danger: "#c62828",
      ok: "#2e7d32",
      signal: "#1565c0",
      connector: "#af6a1a",
      inport: "#1976d2",
      outport: "#ef6c00",
      busbar: "#607d8b",
      goto: "#8e24aa",
      from: "#00897b",
      surfaceInput: "#ffffff",
      surfaceHover: "#e4eaf1",
      surfaceActive: "#d7e4f5",
      titlebarBg: "#e8ecf1",
      activityBg: "#dde3ea",
      statusbarBg: "#0b6bcb",
      statusbarFg: "#ffffff",
      canvasBg: "#fafbfc",
      canvasGrid: "#d0d7e0",
      btnBg: "#e8ecf1",
      btnHover: "#d7dee6",
      winCloseHover: "#e81123",
      progress: "#0b6bcb",
      progressEnd: "#3d8bfd",
      overlay: "rgba(20, 28, 40, 0.35)",
      shadow: "rgba(20, 28, 40, 0.18)",
      blockSignal: "#e3effa",
      blockElectrical: "#f7eadc",
      blockMultibody: "#e4f2e7",
      blockSubsystem: "#e6f3e6",
      blockFmu: "#ebe8f8",
      blockSubBorder: "#4caf50",
      blockFmuBorder: "#7e57c2",
    },
  },
  {
    id: "classic",
    labelKey: "themeClassic",
    tokens: {
      bg: "#1e2430",
      bgElevated: "#161b26",
      panel: "#252c3a",
      panelAlt: "#1c2230",
      border: "#3e4a5c",
      borderSubtle: "#2f3848",
      text: "#edf1f7",
      muted: "#95a0b0",
      accent: "#4ea1ff",
      accentFg: "#0a1220",
      danger: "#ef6b78",
      ok: "#45c98a",
      signal: "#6ab0e8",
      connector: "#d0924a",
      inport: "#4a9ad9",
      outport: "#e8924a",
      busbar: "#7a8ba0",
      goto: "#a878c8",
      from: "#45b090",
      surfaceInput: "#141925",
      surfaceHover: "#313a4c",
      surfaceActive: "#36465c",
      titlebarBg: "#161b26",
      activityBg: "#121722",
      statusbarBg: "#1f6feb",
      statusbarFg: "#ffffff",
      canvasBg: "#121722",
      canvasGrid: "#2f3848",
      btnBg: "#313a4c",
      btnHover: "#3a455a",
      winCloseHover: "#e81123",
      progress: "#1f6feb",
      progressEnd: "#4ea1ff",
      overlay: "rgba(0, 0, 0, 0.5)",
      shadow: "rgba(0, 0, 0, 0.45)",
      blockSignal: "#1f3348",
      blockElectrical: "#3d3226",
      blockMultibody: "#1f3528",
      blockSubsystem: "#243a28",
      blockFmu: "#2a2c42",
      blockSubBorder: "#5cb85c",
      blockFmuBorder: "#8a7ad8",
    },
  },
  {
    id: "highContrast",
    labelKey: "themeHighContrast",
    tokens: {
      bg: "#000000",
      bgElevated: "#000000",
      panel: "#0a0a0a",
      panelAlt: "#000000",
      border: "#ffffff",
      borderSubtle: "#c0c0c0",
      text: "#ffffff",
      muted: "#d0d0d0",
      accent: "#ffff00",
      accentFg: "#000000",
      danger: "#ff5555",
      ok: "#00ff66",
      signal: "#00e5ff",
      connector: "#ffaa00",
      inport: "#00b0ff",
      outport: "#ff9100",
      busbar: "#aaaaaa",
      goto: "#e040fb",
      from: "#00e676",
      surfaceInput: "#000000",
      surfaceHover: "#222222",
      surfaceActive: "#333333",
      titlebarBg: "#000000",
      activityBg: "#000000",
      statusbarBg: "#000000",
      statusbarFg: "#ffff00",
      canvasBg: "#000000",
      canvasGrid: "#555555",
      btnBg: "#000000",
      btnHover: "#222222",
      winCloseHover: "#ff0000",
      progress: "#ffff00",
      progressEnd: "#ffffff",
      overlay: "rgba(0, 0, 0, 0.75)",
      shadow: "rgba(255, 255, 255, 0.25)",
      blockSignal: "#001a33",
      blockElectrical: "#331a00",
      blockMultibody: "#002211",
      blockSubsystem: "#003300",
      blockFmu: "#1a0033",
      blockSubBorder: "#00ff66",
      blockFmuBorder: "#cc66ff",
    },
  },
];

const TOKEN_CSS: Record<keyof ThemeTokens, string> = {
  bg: "--bg",
  bgElevated: "--bg-elevated",
  panel: "--panel",
  panelAlt: "--panel-alt",
  border: "--border",
  borderSubtle: "--border-subtle",
  text: "--text",
  muted: "--muted",
  accent: "--accent",
  accentFg: "--accent-fg",
  danger: "--danger",
  ok: "--ok",
  signal: "--signal",
  connector: "--connector",
  inport: "--inport",
  outport: "--outport",
  busbar: "--busbar",
  goto: "--goto",
  from: "--from",
  surfaceInput: "--surface-input",
  surfaceHover: "--surface-hover",
  surfaceActive: "--surface-active",
  titlebarBg: "--titlebar-bg",
  activityBg: "--activity-bg",
  statusbarBg: "--statusbar-bg",
  statusbarFg: "--statusbar-fg",
  canvasBg: "--canvas-bg",
  canvasGrid: "--canvas-grid",
  btnBg: "--btn-bg",
  btnHover: "--btn-hover",
  winCloseHover: "--win-close-hover",
  progress: "--progress",
  progressEnd: "--progress-end",
  overlay: "--overlay",
  shadow: "--shadow",
  blockSignal: "--block-signal",
  blockElectrical: "--block-electrical",
  blockMultibody: "--block-multibody",
  blockSubsystem: "--block-subsystem",
  blockFmu: "--block-fmu",
  blockSubBorder: "--block-sub-border",
  blockFmuBorder: "--block-fmu-border",
};

export function resolveThemeId(raw: unknown): ThemeId {
  if (raw === "dark" || raw === "light" || raw === "classic" || raw === "highContrast") {
    return raw;
  }
  return DEFAULT_THEME_ID;
}

export function getTheme(id: ThemeId): ThemeDefinition {
  return CLASSIC_THEMES.find((t) => t.id === id) ?? CLASSIC_THEMES[0]!;
}

export function applyThemeToDom(id: ThemeId): void {
  const theme = getTheme(id);
  const root = document.documentElement;
  const scheme = id === "light" ? "light" : "dark";
  root.dataset.theme = theme.id;
  root.style.colorScheme = scheme;
  const meta = document.querySelector('meta[name="color-scheme"]');
  if (meta) meta.setAttribute("content", scheme);
  for (const [key, cssName] of Object.entries(TOKEN_CSS) as [keyof ThemeTokens, string][]) {
    root.style.setProperty(cssName, theme.tokens[key]);
  }
  void getCurrentWindow()
    .setTheme(scheme)
    .catch(() => {
      /* browser preview */
    });
}

export function readCssVar(name: string, fallback = ""): string {
  if (typeof document === "undefined") return fallback;
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return v || fallback;
}

export function readCssPx(name: string, fallback: number): number {
  const n = Number.parseFloat(readCssVar(name, String(fallback)));
  return Number.isFinite(n) ? n : fallback;
}
