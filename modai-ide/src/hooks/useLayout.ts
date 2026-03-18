import { useState, useCallback, useRef, useEffect } from "react";
import { PREFS_KEYS, readPref, writePref, type DefaultWorkspace } from "../utils/prefsConstants";

export interface LayoutState {
  showLeftSidebar: boolean;
  showRightPanel: boolean;
  showBottomPanel: boolean;
  leftSidebarWidth: number;
  rightPanelWidth: number;
  bottomPanelHeight: number;
  leftSidebarTab: "explorer" | "sourceControl" | "search";
  rightPanelTab: "ai" | "diff";
  graphExpanded: boolean;
  showProjectMenu: boolean;
  showSettings: boolean;
  workspaceMode: "modelica" | "component-library" | "compiler-iterate";
}

const parseLayoutNum = (s: string | null, defaultVal: number) => {
  const n = s ? parseInt(s, 10) : NaN;
  return Number.isNaN(n) ? defaultVal : n;
};

function readRestoreLayout(): boolean {
  return readPref(PREFS_KEYS.restoreLayout, (s) => s === "true", true);
}

export function useLayout() {
  const [showLeftSidebar, setShowLeftSidebar] = useState(() =>
    readRestoreLayout() ? readPref(PREFS_KEYS.showLeftSidebar, (s) => s !== "false", true) : true
  );
  const [showRightPanel, setShowRightPanel] = useState(() =>
    readRestoreLayout() ? readPref(PREFS_KEYS.showRightPanel, (s) => s !== "false", true) : true
  );
  const [showBottomPanel, setShowBottomPanel] = useState(() =>
    readRestoreLayout() ? readPref(PREFS_KEYS.showBottomPanel, (s) => s !== "false", true) : true
  );
  const [leftSidebarWidth, setLeftSidebarWidth] = useState(() =>
    readRestoreLayout() ? readPref(PREFS_KEYS.leftSidebarWidth, (s) => parseLayoutNum(s, 240), 240) : 240
  );
  const [rightPanelWidth, setRightPanelWidth] = useState(() =>
    readRestoreLayout() ? readPref(PREFS_KEYS.rightPanelWidth, (s) => parseLayoutNum(s, 360), 360) : 360
  );
  const [bottomPanelHeight, setBottomPanelHeight] = useState(() =>
    readRestoreLayout() ? readPref(PREFS_KEYS.bottomPanelHeight, (s) => parseLayoutNum(s, 200), 200) : 200
  );
  const [leftSidebarTab, setLeftSidebarTab] = useState<"explorer" | "sourceControl" | "search">(() =>
    readRestoreLayout()
      ? readPref(PREFS_KEYS.leftSidebarTab, (s) =>
          s === "sourceControl" || s === "search" ? s : "explorer", "explorer")
      : "explorer"
  );
  const [rightPanelTab, setRightPanelTab] = useState<"ai" | "diff">(() =>
    readRestoreLayout() ? readPref(PREFS_KEYS.rightPanelTab, (s) => (s === "diff" ? "diff" : "ai"), "ai") : "ai"
  );
  const [graphExpanded, setGraphExpanded] = useState(() =>
    readRestoreLayout() ? readPref(PREFS_KEYS.graphExpanded, (s) => s === "true", false) : false
  );
  const [showProjectMenu, setShowProjectMenu] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [openSettingsToGroup, setOpenSettingsToGroup] = useState<string | null>(null);
  const [defaultWorkspace, setDefaultWorkspaceState] = useState<DefaultWorkspace>(() =>
    readPref(
      PREFS_KEYS.defaultWorkspace,
      (s) =>
        s === "component-library" || s === "compiler-iterate" ? s : "modelica",
      "modelica"
    )
  );
  const [restoreLayout, setRestoreLayoutState] = useState(readRestoreLayout);
  const [workspaceMode, setWorkspaceMode] = useState<"modelica" | "component-library" | "compiler-iterate">(
    () =>
      readPref(
        PREFS_KEYS.defaultWorkspace,
        (s) =>
          s === "component-library" || s === "compiler-iterate" ? s : "modelica",
        "modelica"
      )
  );

  const [theme, setTheme] = useState<"dark" | "light">(() => {
    try {
      const s = localStorage.getItem("modai-theme");
      return s === "light" ? "light" : "dark";
    } catch {
      return "dark";
    }
  });

  const [fontUi, setFontUi] = useState<"chinese" | "code">(() => {
    try {
      const s = localStorage.getItem("modai-font-ui");
      return s === "code" ? "code" : "chinese";
    } catch {
      return "chinese";
    }
  });

  const [fontSizePercent, setFontSizePercent] = useState<90 | 100 | 110 | 120>(() => {
    try {
      const s = localStorage.getItem("modai-font-size-percent");
      const n = s ? parseInt(s, 10) : 100;
      return [90, 100, 110, 120].includes(n) ? (n as 90 | 100 | 110 | 120) : 100;
    } catch {
      return 100;
    }
  });

  const [uiColorScheme, setUiColorScheme] = useState<"default" | "classic">(() => {
    try {
      const s = localStorage.getItem("modai-ui-color-scheme");
      return s === "classic" ? "classic" : "default";
    } catch {
      return "default";
    }
  });

  const [panelHeaderHeight, setPanelHeaderHeightState] = useState<number>(() =>
    readPref(PREFS_KEYS.layoutPanelHeaderHeight, (s) => parseLayoutNum(s, 32), 32)
  );
  const [toolbarBtnSize, setToolbarBtnSizeState] = useState<number>(() =>
    readPref(PREFS_KEYS.layoutToolbarBtnSize, (s) => parseLayoutNum(s, 26), 26)
  );
  const [toolbarGap, setToolbarGapState] = useState<number>(() =>
    readPref(PREFS_KEYS.layoutToolbarGap, (s) => parseLayoutNum(s, 8), 8)
  );

  const [lang, setLangState] = useState<"en" | "zh">(() =>
    readPref(PREFS_KEYS.lang, (s) => (s === "en" ? "en" : "zh"), "zh")
  );

  const resizingRef = useRef<{
    type: "left" | "right" | "bottom";
    startX: number;
    startY: number;
    startSize: number;
  } | null>(null);

  useEffect(() => {
    if (!restoreLayout) return;
    writePref(PREFS_KEYS.showLeftSidebar, showLeftSidebar ? "true" : "false");
    writePref(PREFS_KEYS.showRightPanel, showRightPanel ? "true" : "false");
    writePref(PREFS_KEYS.showBottomPanel, showBottomPanel ? "true" : "false");
    writePref(PREFS_KEYS.leftSidebarWidth, String(leftSidebarWidth));
    writePref(PREFS_KEYS.rightPanelWidth, String(rightPanelWidth));
    writePref(PREFS_KEYS.bottomPanelHeight, String(bottomPanelHeight));
    writePref(PREFS_KEYS.leftSidebarTab, leftSidebarTab);
    writePref(PREFS_KEYS.rightPanelTab, rightPanelTab);
    writePref(PREFS_KEYS.graphExpanded, graphExpanded ? "true" : "false");
  }, [
    restoreLayout,
    showLeftSidebar,
    showRightPanel,
    showBottomPanel,
    leftSidebarWidth,
    rightPanelWidth,
    bottomPanelHeight,
    leftSidebarTab,
    rightPanelTab,
    graphExpanded,
  ]);

  const startResizeLeft = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      resizingRef.current = { type: "left", startX: e.clientX, startY: 0, startSize: leftSidebarWidth };
      const onMove = (ev: MouseEvent) => {
        const r = resizingRef.current;
        if (!r || r.type !== "left") return;
        const delta = ev.clientX - r.startX;
        setLeftSidebarWidth(Math.min(480, Math.max(160, r.startSize + delta)));
      };
      const onUp = () => {
        resizingRef.current = null;
        window.removeEventListener("mousemove", onMove);
        window.removeEventListener("mouseup", onUp);
      };
      window.addEventListener("mousemove", onMove);
      window.addEventListener("mouseup", onUp);
    },
    [leftSidebarWidth]
  );

  const startResizeRight = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      resizingRef.current = { type: "right", startX: e.clientX, startY: 0, startSize: rightPanelWidth };
      const onMove = (ev: MouseEvent) => {
        const r = resizingRef.current;
        if (!r || r.type !== "right") return;
        const delta = ev.clientX - r.startX;
        setRightPanelWidth(Math.min(600, Math.max(280, r.startSize - delta)));
      };
      const onUp = () => {
        resizingRef.current = null;
        window.removeEventListener("mousemove", onMove);
        window.removeEventListener("mouseup", onUp);
      };
      window.addEventListener("mousemove", onMove);
      window.addEventListener("mouseup", onUp);
    },
    [rightPanelWidth]
  );

  const setLang = useCallback((next: "en" | "zh") => {
    setLangState(next);
    writePref(PREFS_KEYS.lang, next);
  }, []);

  const setDefaultWorkspace = useCallback((next: DefaultWorkspace) => {
    setDefaultWorkspaceState(next);
    writePref(PREFS_KEYS.defaultWorkspace, next);
  }, []);

  const setRestoreLayout = useCallback((next: boolean) => {
    setRestoreLayoutState(next);
    writePref(PREFS_KEYS.restoreLayout, next ? "true" : "false");
  }, []);

  const setPanelHeaderHeight = useCallback((next: number) => {
    setPanelHeaderHeightState(next);
    writePref(PREFS_KEYS.layoutPanelHeaderHeight, String(next));
  }, []);
  const setToolbarBtnSize = useCallback((next: number) => {
    setToolbarBtnSizeState(next);
    writePref(PREFS_KEYS.layoutToolbarBtnSize, String(next));
  }, []);
  const setToolbarGap = useCallback((next: number) => {
    setToolbarGapState(next);
    writePref(PREFS_KEYS.layoutToolbarGap, String(next));
  }, []);

  const startResizeBottom = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      resizingRef.current = { type: "bottom", startX: 0, startY: e.clientY, startSize: bottomPanelHeight };
      const onMove = (ev: MouseEvent) => {
        const r = resizingRef.current;
        if (!r || r.type !== "bottom") return;
        const delta = ev.clientY - r.startY;
        setBottomPanelHeight(Math.min(400, Math.max(120, r.startSize - delta)));
      };
      const onUp = () => {
        resizingRef.current = null;
        window.removeEventListener("mousemove", onMove);
        window.removeEventListener("mouseup", onUp);
      };
      window.addEventListener("mousemove", onMove);
      window.addEventListener("mouseup", onUp);
    },
    [bottomPanelHeight]
  );

  return {
    showLeftSidebar, setShowLeftSidebar,
    showRightPanel, setShowRightPanel,
    showBottomPanel, setShowBottomPanel,
    leftSidebarWidth, rightPanelWidth, bottomPanelHeight,
    leftSidebarTab, setLeftSidebarTab,
    rightPanelTab, setRightPanelTab,
    graphExpanded, setGraphExpanded,
    showProjectMenu, setShowProjectMenu,
    showSettings, setShowSettings,
    openSettingsToGroup, setOpenSettingsToGroup,
    workspaceMode, setWorkspaceMode,
    theme, setTheme,
    fontUi, setFontUi,
    fontSizePercent, setFontSizePercent,
    uiColorScheme, setUiColorScheme,
    panelHeaderHeight, setPanelHeaderHeight,
    toolbarBtnSize, setToolbarBtnSize,
    toolbarGap, setToolbarGap,
    lang, setLang,
    defaultWorkspace, setDefaultWorkspace,
    restoreLayout, setRestoreLayout,
    startResizeLeft, startResizeRight, startResizeBottom,
  };
}
