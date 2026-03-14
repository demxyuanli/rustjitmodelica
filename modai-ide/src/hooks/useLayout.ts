import { useState, useCallback, useRef } from "react";

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

export function useLayout() {
  const [showLeftSidebar, setShowLeftSidebar] = useState(true);
  const [showRightPanel, setShowRightPanel] = useState(true);
  const [showBottomPanel, setShowBottomPanel] = useState(true);
  const [leftSidebarWidth, setLeftSidebarWidth] = useState(240);
  const [rightPanelWidth, setRightPanelWidth] = useState(360);
  const [bottomPanelHeight, setBottomPanelHeight] = useState(200);
  const [leftSidebarTab, setLeftSidebarTab] = useState<"explorer" | "sourceControl" | "search">("explorer");
  const [rightPanelTab, setRightPanelTab] = useState<"ai" | "diff">("ai");
  const [graphExpanded, setGraphExpanded] = useState(false);
  const [showProjectMenu, setShowProjectMenu] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [workspaceMode, setWorkspaceMode] = useState<"modelica" | "component-library" | "compiler-iterate">("modelica");

  const [theme, setTheme] = useState<"dark" | "light">(() => {
    try {
      const s = localStorage.getItem("modai-theme");
      return s === "light" ? "light" : "dark";
    } catch {
      return "dark";
    }
  });

  const [lang, setLangState] = useState<"en" | "zh">("zh");

  const resizingRef = useRef<{
    type: "left" | "right" | "bottom";
    startX: number;
    startY: number;
    startSize: number;
  } | null>(null);

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
    workspaceMode, setWorkspaceMode,
    theme, setTheme,
    lang, setLangState,
    startResizeLeft, startResizeRight, startResizeBottom,
  };
}
