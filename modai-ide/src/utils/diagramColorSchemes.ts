export interface DiagramColorScheme {
  id: string;
  labelKey: string;
  connectorColors: Record<string, string>;
  chartPaletteLight: string[];
  chartPaletteDark: string[];
  diagramPrimary?: string;
}

const DEFAULT_CONNECTOR_KEYS = [
  "mechanical",
  "electrical",
  "thermal",
  "fluid",
  "signal_input",
  "signal_output",
] as const;

function makeConnectorColors(entries: Record<string, string>): Record<string, string> {
  const base: Record<string, string> = {
    mechanical: "#555",
    electrical: "#2563eb",
    thermal: "#dc2626",
    fluid: "#0891b2",
    signal_input: "#16a34a",
    signal_output: "#ca8a04",
  };
  return { ...base, ...entries };
}

export const DIAGRAM_COLOR_SCHEMES: DiagramColorScheme[] = [
  {
    id: "default",
    labelKey: "schemeDefault",
    connectorColors: makeConnectorColors({}),
    chartPaletteLight: ["#2563eb", "#dc2626", "#16a34a", "#9333ea", "#ea580c", "#0891b2", "#4f46e5", "#ca8a04"],
    chartPaletteDark: ["#60a5fa", "#f87171", "#4ade80", "#c084fc", "#fb923c", "#22d3ee", "#818cf8", "#facc15"],
  },
  {
    id: "ocean",
    labelKey: "schemeOcean",
    connectorColors: makeConnectorColors({
      mechanical: "#475569",
      electrical: "#0ea5e9",
      thermal: "#f97316",
      fluid: "#06b6d4",
      signal_input: "#14b8a6",
      signal_output: "#8b5cf6",
    }),
    chartPaletteLight: ["#0284c7", "#0d9488", "#059669", "#2563eb", "#7c3aed", "#c026d3", "#db2777", "#dc2626"],
    chartPaletteDark: ["#38bdf8", "#2dd4bf", "#34d399", "#60a5fa", "#a78bfa", "#e879f9", "#f472b6", "#f87171"],
    diagramPrimary: "#0ea5e9",
  },
  {
    id: "forest",
    labelKey: "schemeForest",
    connectorColors: makeConnectorColors({
      mechanical: "#57534e",
      electrical: "#65a30d",
      thermal: "#ea580c",
      fluid: "#0d9488",
      signal_input: "#16a34a",
      signal_output: "#ca8a04",
    }),
    chartPaletteLight: ["#15803d", "#166534", "#4d7c0f", "#65a30d", "#84cc16", "#a16207", "#b45309", "#c2410c"],
    chartPaletteDark: ["#22c55e", "#4ade80", "#84cc16", "#a3e635", "#bef264", "#fbbf24", "#f97316", "#fb923c"],
    diagramPrimary: "#16a34a",
  },
  {
    id: "vibrant",
    labelKey: "schemeVibrant",
    connectorColors: makeConnectorColors({
      mechanical: "#6b7280",
      electrical: "#6366f1",
      thermal: "#ef4444",
      fluid: "#ec4899",
      signal_input: "#10b981",
      signal_output: "#f59e0b",
    }),
    chartPaletteLight: ["#6366f1", "#ec4899", "#10b981", "#f59e0b", "#ef4444", "#8b5cf6", "#06b6d4", "#84cc16"],
    chartPaletteDark: ["#818cf8", "#f472b6", "#34d399", "#fbbf24", "#f87171", "#a78bfa", "#22d3ee", "#a3e635"],
    diagramPrimary: "#8b5cf6",
  },
  {
    id: "monochrome",
    labelKey: "schemeMonochrome",
    connectorColors: makeConnectorColors({
      mechanical: "#525252",
      electrical: "#737373",
      thermal: "#a3a3a3",
      fluid: "#737373",
      signal_input: "#525252",
      signal_output: "#a3a3a3",
    }),
    chartPaletteLight: ["#404040", "#525252", "#737373", "#a3a3a3", "#737373", "#525252", "#404040", "#262626"],
    chartPaletteDark: ["#a3a3a3", "#737373", "#525252", "#404040", "#737373", "#a3a3a3", "#d4d4d4", "#e5e5e5"],
    diagramPrimary: "#737373",
  },
];

const STORAGE_KEY = "modai-diagram-color-scheme";
const STORAGE_KEY_OVERRIDES = "modai-diagram-color-overrides";

export type DiagramColorOverrides = Partial<{
  connectorColors: Partial<Record<string, string>>;
  chartPaletteLight: string[];
  chartPaletteDark: string[];
  diagramPrimary: string;
}>;

function loadOverrides(): Record<string, DiagramColorOverrides> {
  if (typeof window === "undefined") return {};
  try {
    const raw = localStorage.getItem(STORAGE_KEY_OVERRIDES);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Record<string, DiagramColorOverrides>;
    return typeof parsed === "object" && parsed !== null ? parsed : {};
  } catch {
    return {};
  }
}

function saveOverrides(data: Record<string, DiagramColorOverrides>): void {
  try {
    localStorage.setItem(STORAGE_KEY_OVERRIDES, JSON.stringify(data));
    window.dispatchEvent(new CustomEvent("modai-diagram-scheme-change"));
  } catch {
    /* ignore */
  }
}

function mergeScheme(base: DiagramColorScheme, overrides: DiagramColorOverrides | undefined): DiagramColorScheme {
  if (!overrides) return base;
  const connectorMerged = { ...base.connectorColors };
  if (overrides.connectorColors) {
    for (const [k, v] of Object.entries(overrides.connectorColors)) {
      if (v !== undefined) connectorMerged[k] = v;
    }
  }
  return {
    id: base.id,
    labelKey: base.labelKey,
    connectorColors: connectorMerged,
    chartPaletteLight: overrides.chartPaletteLight ?? base.chartPaletteLight,
    chartPaletteDark: overrides.chartPaletteDark ?? base.chartPaletteDark,
    diagramPrimary: overrides.diagramPrimary !== undefined ? overrides.diagramPrimary : base.diagramPrimary,
  };
}

export function getActiveSchemeId(): string {
  if (typeof window === "undefined") return "default";
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored && DIAGRAM_COLOR_SCHEMES.some((s) => s.id === stored)) return stored;
  } catch {
    /* ignore */
  }
  return "default";
}

export function setActiveSchemeId(id: string): void {
  if (!DIAGRAM_COLOR_SCHEMES.some((s) => s.id === id)) return;
  try {
    localStorage.setItem(STORAGE_KEY, id);
    window.dispatchEvent(new CustomEvent("modai-diagram-scheme-change"));
  } catch {
    /* ignore */
  }
}

export function getActiveScheme(): DiagramColorScheme {
  const id = getActiveSchemeId();
  const base = DIAGRAM_COLOR_SCHEMES.find((s) => s.id === id) ?? DIAGRAM_COLOR_SCHEMES[0];
  const overrides = loadOverrides()[id];
  return mergeScheme(base, overrides);
}

export function getSchemeById(id: string): DiagramColorScheme | undefined {
  const base = DIAGRAM_COLOR_SCHEMES.find((s) => s.id === id);
  if (!base) return undefined;
  const overrides = loadOverrides()[id];
  return mergeScheme(base, overrides);
}

export function getColorOverrides(schemeId: string): DiagramColorOverrides | undefined {
  return loadOverrides()[schemeId];
}

export function setColorOverrides(schemeId: string, next: DiagramColorOverrides): void {
  const all = loadOverrides();
  const current = all[schemeId] ?? {};
  const merged: DiagramColorOverrides = {
    connectorColors: { ...(current.connectorColors ?? {}), ...(next.connectorColors ?? {}) },
    chartPaletteLight: next.chartPaletteLight ?? current.chartPaletteLight,
    chartPaletteDark: next.chartPaletteDark ?? current.chartPaletteDark,
    diagramPrimary: next.diagramPrimary !== undefined ? next.diagramPrimary : current.diagramPrimary,
  };
  if (merged.connectorColors && Object.keys(merged.connectorColors).length === 0) delete merged.connectorColors;
  if (merged.chartPaletteLight === undefined) delete merged.chartPaletteLight;
  if (merged.chartPaletteDark === undefined) delete merged.chartPaletteDark;
  if (merged.diagramPrimary === undefined) delete merged.diagramPrimary;
  if (Object.keys(merged).length > 0) {
    all[schemeId] = merged;
  } else {
    delete all[schemeId];
  }
  saveOverrides(all);
}

export function setSingleColorOverride(
  schemeId: string,
  kind: "connectorColors" | "chartPaletteLight" | "chartPaletteDark" | "diagramPrimary",
  keyOrIndex: string | number,
  value: string
): void {
  const base = DIAGRAM_COLOR_SCHEMES.find((s) => s.id === schemeId);
  if (!base) return;
  const overrides = loadOverrides()[schemeId] ?? {};
  if (kind === "connectorColors") {
    const key = keyOrIndex as string;
    setColorOverrides(schemeId, { connectorColors: { ...overrides.connectorColors, [key]: value } });
    return;
  }
  if (kind === "diagramPrimary") {
    setColorOverrides(schemeId, { diagramPrimary: value });
    return;
  }
  const arr = kind === "chartPaletteLight"
    ? (getSchemeById(schemeId)?.chartPaletteLight ?? base.chartPaletteLight).slice()
    : (getSchemeById(schemeId)?.chartPaletteDark ?? base.chartPaletteDark).slice();
  const i = typeof keyOrIndex === "number" ? keyOrIndex : parseInt(String(keyOrIndex), 10);
  if (i >= 0 && i < arr.length) {
    arr[i] = value;
    setColorOverrides(schemeId, kind === "chartPaletteLight" ? { chartPaletteLight: arr } : { chartPaletteDark: arr });
  }
}

export function clearColorOverrides(schemeId: string): void {
  const all = loadOverrides();
  delete all[schemeId];
  saveOverrides(all);
}

export function hasColorOverrides(schemeId: string): boolean {
  const o = loadOverrides()[schemeId];
  return o !== undefined && Object.keys(o).length > 0;
}

export { DEFAULT_CONNECTOR_KEYS };
