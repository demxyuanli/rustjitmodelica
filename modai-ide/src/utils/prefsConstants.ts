/**
 * LocalStorage keys for app preferences. All use modai- prefix.
 * Centralized to avoid scattered magic strings.
 */
export const PREFS_KEYS = {
  theme: "modai-theme",
  fontUi: "modai-font-ui",
  fontSizePercent: "modai-font-size-percent",
  uiColorScheme: "modai-ui-color-scheme",
  lang: "modai-lang",
  defaultWorkspace: "modai-default-workspace",
  restoreLayout: "modai-restore-layout",
  lastProjectDir: "modai-last-project-dir",
  recentProjectDirs: "modai-recent-project-dirs",
  showWelcomeOnFirstLaunch: "modai-show-welcome-on-first-launch",
  aiDaily: "modai-ai-daily",
  aiModel: "modai-ai-model",
  diagramColorScheme: "modai-diagram-color-scheme",
  libraryFavorites: "modai-library-favorites",
  layoutPanelHeaderHeight: "modai-layout-panel-header-height",
  layoutToolbarBtnSize: "modai-layout-toolbar-btn-size",
  layoutToolbarGap: "modai-layout-toolbar-gap",
  showLeftSidebar: "modai-layout-show-left-sidebar",
  showRightPanel: "modai-layout-show-right-panel",
  showBottomPanel: "modai-layout-show-bottom-panel",
  leftSidebarWidth: "modai-layout-left-sidebar-width",
  rightPanelWidth: "modai-layout-right-panel-width",
  bottomPanelHeight: "modai-layout-bottom-panel-height",
  leftSidebarTab: "modai-layout-left-sidebar-tab",
  rightPanelTab: "modai-layout-right-panel-tab",
  graphExpanded: "modai-layout-graph-expanded",
} as const;

export type DefaultWorkspace = "modelica" | "component-library" | "compiler-iterate";

export function readPref<T>(key: string, parse: (s: string | null) => T, defaultValue: T): T {
  if (typeof localStorage === "undefined") return defaultValue;
  try {
    return parse(localStorage.getItem(key)) ?? defaultValue;
  } catch {
    return defaultValue;
  }
}

export function writePref(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* ignore */
  }
}
