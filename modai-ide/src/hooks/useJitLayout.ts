import { useState, useCallback, useRef } from "react";

export type JitLeftTab = "source" | "tests" | "links";
export type JitRightTab = "iterate" | "ai";
export type JitBottomTab = "output" | "testResults";
export type JitCenterView = "analytics" | "trace" | "overview" | "map" | "settings";

export interface JitLayoutState {
  showLeftSidebar: boolean;
  showRightPanel: boolean;
  showBottomPanel: boolean;
  leftSidebarWidth: number;
  rightPanelWidth: number;
  bottomPanelHeight: number;
  leftTab: JitLeftTab;
  rightTab: JitRightTab;
  bottomTab: JitBottomTab;
  activeCenterView: JitCenterView | null;
}

export function useJitLayout() {
  const [showLeftSidebar, setShowLeftSidebar] = useState(true);
  const [showRightPanel, setShowRightPanel] = useState(true);
  const [showBottomPanel, setShowBottomPanel] = useState(true);
  const [leftSidebarWidth, setLeftSidebarWidth] = useState(240);
  const [rightPanelWidth, setRightPanelWidth] = useState(380);
  const [bottomPanelHeight, setBottomPanelHeight] = useState(220);
  const [leftTab, setLeftTab] = useState<JitLeftTab>("source");
  const [rightTab, setRightTab] = useState<JitRightTab>("iterate");
  const [bottomTab, setBottomTab] = useState<JitBottomTab>("output");
  const [activeCenterView, setActiveCenterView] = useState<JitCenterView | null>(null);

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
        setLeftSidebarWidth(Math.min(480, Math.max(160, r.startSize + (ev.clientX - r.startX))));
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
        setRightPanelWidth(Math.min(600, Math.max(280, r.startSize - (ev.clientX - r.startX))));
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
        setBottomPanelHeight(Math.min(500, Math.max(120, r.startSize - (ev.clientY - r.startY))));
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
    leftTab, setLeftTab,
    rightTab, setRightTab,
    bottomTab, setBottomTab,
    activeCenterView, setActiveCenterView,
    startResizeLeft, startResizeRight, startResizeBottom,
  };
}
