import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { t } from "../i18n";
import type { GraphicItem, IconDiagramAnnotation } from "./diagramGraphicTypes";
import {
  applyGraphicalDocumentEdits,
  getGraphicalDocumentFromSource,
  GRAPHICAL_DOCUMENT_LOAD_TIMEOUT_MS,
  isDiagramLoadTimeout,
} from "../api/tauri";
import { GraphicalCanvas, type GraphicalMessage } from "./GraphicalCanvas";
import { decodeModelicaDragPayload, MODELICA_DRAG_TYPE } from "./LibrariesBrowser";
import {
  buildDiagramSyncKey,
  DEBOUNCE_MS,
  diagramToNodes,
  documentToDiagram,
  nodesToDiagram,
  uniqueInstanceName,
  type DiagramDocument,
} from "../structureEditor/docSync";
import type { ComponentData, ConnectionData, LayoutPoint } from "../structureEditor/types";
import { createStructureGraphSession, type StructureSnapMode } from "../structureEditor/session";
import { STRUCTURE_GRID_DEFAULT } from "../structureEditor/layoutConstants";
import { JointStructureEditor } from "../structureEditor/JointStructureEditor";
import { IconEditorShell } from "../structureEditor/IconEditorShell";
import { useStepDebug } from "../hooks/useStepDebug";
import { useDiagramSimulation } from "../hooks/useDiagramSimulation";
import { useDiagramViewPaperState } from "../hooks/useDiagramViewPaperState";
import { useDiagramGraphicInteraction } from "../hooks/useDiagramGraphicInteraction";
import { applyDiagramLayout, type DiagramLayoutKind } from "../utils/diagramLayout";
import { DiagramCoordinateStrip } from "./diagram/DiagramCoordinateStrip";
import { DiagramToolbar } from "./diagram/DiagramToolbar";
import { AlignmentToolbar } from "./diagram/AlignmentToolbar";
import { MultiSelectToolbar } from "./diagram/MultiSelectToolbar";
import { LayerPanel } from "./icon/LayerPanel";
import { downloadSvg, downloadPng } from "../utils/graphicExport";
import type { DependencyGraphBehavior } from "../utils/dependencyGraphBehavior";
export type { LayoutPoint } from "../structureEditor/types";

export interface DiagramViewProps {
  source: string;
  projectDir: string | null;
  relativeFilePath?: string | null;
  onContentChange?: (newSource: string) => void;
  readOnly?: boolean;
  onNavigateToType?: (typeName: string, libraryId?: string) => void;
  /** Alt+double-click on a component: open class source (read-only tab) when set. */
  onOpenTypeSource?: (typeName: string, libraryId?: string) => void;
  mode?: "diagram" | "icon";
  focusSymbolQuery?: string | null;
  libraryRefreshToken?: number;
  onOpenDependencyGraphSettings?: () => void;
  dependencyGraphBehavior: DependencyGraphBehavior;
}

export function DiagramView({
  source,
  projectDir,
  relativeFilePath,
  onContentChange,
  readOnly = false,
  onNavigateToType,
  onOpenTypeSource,
  mode = "diagram",
  focusSymbolQuery = null,
  libraryRefreshToken = 0,
  onOpenDependencyGraphSettings,
  dependencyGraphBehavior,
}: DiagramViewProps) {
  const SYNC_STATUS_PREFIX = "[sync-status] ";
  const SYNC_SLOW_HINT_MS = 2000;
  const SYNC_HEARTBEAT_MS = 5000;
  const sessionRef = useRef<ReturnType<typeof createStructureGraphSession> | null>(null);
  if (!sessionRef.current) sessionRef.current = createStructureGraphSession();
  const session = sessionRef.current;

  const [error, setError] = useState<string | null>(null);
  const [loadTimedOut, setLoadTimedOut] = useState(false);
  const [loading, setLoading] = useState(true);
  const [loadingMessage, setLoadingMessage] = useState(() => t("diagramLoading"));
  const [conflictPending, setConflictPending] = useState<DiagramDocument | null>(null);
  const [messages, setMessages] = useState<GraphicalMessage[]>([]);
  const [structureSnapMode, setStructureSnapMode] = useState<StructureSnapMode>("grid");
  const [structureGridSize, setStructureGridSize] = useState(STRUCTURE_GRID_DEFAULT);
  const [pointerLocal, setPointerLocal] = useState<{ x: number; y: number } | null>(null);

  const sourceRef = useRef(source);
  sourceRef.current = source;
  const projectDirRef = useRef(projectDir);
  projectDirRef.current = projectDir;
  const filePathRef = useRef(relativeFilePath);
  filePathRef.current = relativeFilePath;
  const onContentChangeRef = useRef(onContentChange);
  onContentChangeRef.current = onContentChange;
  const modeRef = useRef(mode);
  modeRef.current = mode;

  const handleDoubleClick = useCallback(
    (typeName: string, libraryId?: string) => {
      onNavigateToType?.(typeName, libraryId);
    },
    [onNavigateToType],
  );

  const handleDoubleClickRef = useRef(handleDoubleClick);
  handleDoubleClickRef.current = handleDoubleClick;

  const sessionRevision = useSyncExternalStore(
    session.subscribe,
    () => session.getRevision(),
    () => session.getRevision(),
  );

  const diagram = useMemo(() => {
    void sessionRevision;
    const doc = session.getDocument();
    return doc ? documentToDiagram(doc) : null;
  }, [session, sessionRevision]);

  const activeGraphics =
    mode === "icon" ? (diagram?.iconAnnotation?.graphics ?? []) : (diagram?.diagramAnnotation?.graphics ?? []);

  const setGraphicsForActiveMode = useCallback(
    (graphics: GraphicItem[]) => {
      session.setGraphicsForMode(modeRef.current, graphics);
    },
    [session],
  );

  const {
    selectedGraphicPath,
    setSelectedGraphicPath,
    selectedGraphicIndices,
    handleUpdateGraphic,
    handleRectangleToPolygon,
    handleAddGraphic,
    handleDeleteGraphic,
    handleSelectGraphic,
    handleToggleLayerHidden,
    handleToggleLayerLocked,
    handleReorderGraphics,
    handleGroupGraphics,
    handleUngroupGraphics,
    handleAlign,
    handleDistribute,
    handleDuplicate,
    handleDeleteSelected,
  } = useDiagramGraphicInteraction(mode, activeGraphics, setGraphicsForActiveMode);

  const {
    paperHandle,
    setPaperHandle,
    showMiniMap,
    setShowMiniMap,
    gridEnabled,
    setGridEnabled,
    gridSize,
    setGridSize,
    showGrid,
    setShowGrid,
  } = useDiagramViewPaperState();

  const loadGraphGenerationRef = useRef(0);
  // Track the last source string we ourselves emitted via syncToSource so we
  // can skip the heavy reload round-trip when the parent feeds it back to us.
  const lastSelfEmittedSourceRef = useRef<string | null>(null);
  const sourceLoadTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const stepDebug = useStepDebug();
  const simOverlay = useDiagramSimulation(stepDebug);

  const handleStartDebug = useCallback(() => {
    stepDebug.startSession(sourceRef.current, diagram?.modelName, projectDirRef.current);
  }, [diagram?.modelName, stepDebug]);

  const handleApplyLayout = useCallback(
    (kind: DiagramLayoutKind) => {
      const { nodes, links } = session.getNodesLinksForCanvas(handleDoubleClickRef.current);
      if (nodes.length === 0) return;
      const nodeIds = nodes.map((n) => n.id);
      const edges = links.map((l) => ({ source: l.source, target: l.target }));
      const positions = applyDiagramLayout(kind, { nodeIds, edges });
      session.setSkipNextUndoPush();
      session.applyLayoutPositions(positions);
    },
    [session],
  );

  useEffect(() => {
    if (readOnly) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      if (e.key === "z" && !e.shiftKey && session.canUndo()) {
        e.preventDefault();
        e.stopPropagation();
        session.undo();
      } else if (((e.key === "z" && e.shiftKey) || e.key === "y") && session.canRedo()) {
        e.preventDefault();
        e.stopPropagation();
        session.redo();
      }
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [readOnly, session, sessionRevision]);

  useEffect(() => {
    // Avoid the reload round-trip when the parent is just echoing back the
    // source we produced via syncToSource. The session is already in sync,
    // and re-running getGraphicalDocumentFromSource for large models can
    // take seconds and shows a loading overlay over the canvas.
    if (lastSelfEmittedSourceRef.current !== null && source === lastSelfEmittedSourceRef.current) {
      return;
    }
    // Debounce: wait 300ms before firing the backend parse so rapid typing
    // doesn't trigger a heavy reload per keystroke.
    if (sourceLoadTimerRef.current) {
      clearTimeout(sourceLoadTimerRef.current);
      sourceLoadTimerRef.current = null;
    }
    const timer = window.setTimeout(() => {
      let cancelled = false;
      const gen = ++loadGraphGenerationRef.current;
      setLoading(true);
      setError(null);
      setLoadTimedOut(false);
      setConflictPending(null);
      setLoadingMessage(t("diagramLoading"));
      const slowHintTimer = window.setTimeout(() => {
        if (!cancelled && gen === loadGraphGenerationRef.current) {
          setLoadingMessage(t("diagramLoadingSlowHint"));
        }
      }, 2000);
      getGraphicalDocumentFromSource<IconDiagramAnnotation, ComponentData, ConnectionData>(
        source,
        projectDir,
        relativeFilePath,
        {
          timeoutMs: GRAPHICAL_DOCUMENT_LOAD_TIMEOUT_MS,
          onTimeout: () => {
            cancelled = true;
          },
        },
      )
        .then((data) => {
          if (cancelled || gen !== loadGraphGenerationRef.current) return;
          const { nodes: curNodes, links: curLinks } = session.getNodesLinksForCanvas(handleDoubleClickRef.current);
          const current = nodesToDiagram(curNodes, curLinks);
          const hasCurrentState = current.components.length > 0 || current.connections.length > 0;
          const sameComponents =
            current.components.length === data.components.length &&
            current.components.every(
              (c, i) =>
                data.components[i] &&
                c.name === data.components[i].name &&
                c.typeName === data.components[i].typeName,
            );
          const sameConnections =
            current.connections.length === data.connections.length &&
            current.connections.every(
              (c, i) =>
                data.connections[i] && c.from === data.connections[i].from && c.to === data.connections[i].to,
            );
          const inSync = sameComponents && sameConnections;
          if (!hasCurrentState || inSync) {
            session.loadFromServer(data, true);
            setSelectedGraphicPath(null);
            setConflictPending(null);
            const nlLoaded = diagramToNodes(documentToDiagram(data), handleDoubleClickRef.current);
            lastAppliedRef.current = buildDiagramSyncKey(nlLoaded.nodes, nlLoaded.links, data.graphical);
          } else {
            setConflictPending(data);
          }
        })
        .catch((err) => {
          if (gen !== loadGraphGenerationRef.current) return;
          if (isDiagramLoadTimeout(err)) {
            setLoadTimedOut(true);
            setError(String(err));
            setConflictPending(null);
            session.clearDocument();
            return;
          }
          if (cancelled) return;
          setError(String(err));
          setConflictPending(null);
          session.clearDocument();
        })
        .finally(() => {
          clearTimeout(slowHintTimer);
          if (gen === loadGraphGenerationRef.current) {
            setLoading(false);
          }
        });
    }, 300);
    sourceLoadTimerRef.current = timer;
    return () => {
      clearTimeout(timer);
      sourceLoadTimerRef.current = null;
    };
  }, [source, session]);

  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastAppliedRef = useRef<string>("");
  const activeSyncTokenRef = useRef(0);
  const activeSyncSlowTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const activeSyncHeartbeatRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const setSyncStatusMessage = useCallback((severity: GraphicalMessage["severity"], text: string) => {
    const payload = `${SYNC_STATUS_PREFIX}${text}`;
    setMessages((prev) => [{ severity, text: payload } as GraphicalMessage, ...prev.filter((m) => !m.text.startsWith(SYNC_STATUS_PREFIX))].slice(0, 20));
  }, []);

  const clearSyncStatusMessage = useCallback(() => {
    setMessages((prev) => prev.filter((m) => !m.text.startsWith(SYNC_STATUS_PREFIX)));
  }, []);

  const clearSyncTimers = useCallback(() => {
    if (activeSyncSlowTimerRef.current) {
      clearTimeout(activeSyncSlowTimerRef.current);
      activeSyncSlowTimerRef.current = null;
    }
    if (activeSyncHeartbeatRef.current) {
      clearInterval(activeSyncHeartbeatRef.current);
      activeSyncHeartbeatRef.current = null;
    }
  }, []);

  const syncToSource = useCallback(() => {
    if (readOnly || !onContentChangeRef.current) return;
    const doc = session.getDocument();
    if (!doc) return;
    const { nodes, links } = session.getNodesLinksForCanvas(handleDoubleClickRef.current);
    const key = buildDiagramSyncKey(nodes, links, doc.graphical);
    if (key === lastAppliedRef.current) return;
    const { components, connections, layout } = nodesToDiagram(nodes, links);
    const byName = new Map(doc.components.map((c) => [c.name, c]));
    const mergedComponents = components.map((c) => {
      const full = byName.get(c.name);
      return full ? { ...full, ...c } : c;
    });
    const previousKey = lastAppliedRef.current;
    lastAppliedRef.current = key;
    const syncToken = activeSyncTokenRef.current + 1;
    activeSyncTokenRef.current = syncToken;
    const syncStart = Date.now();
    let showedSlowStatus = false;
    clearSyncTimers();
    clearSyncStatusMessage();
    activeSyncSlowTimerRef.current = setTimeout(() => {
      if (activeSyncTokenRef.current !== syncToken) return;
      showedSlowStatus = true;
      setSyncStatusMessage("info", "Diagram sync is still running...");
      activeSyncHeartbeatRef.current = setInterval(() => {
        if (activeSyncTokenRef.current !== syncToken) return;
        const elapsedSec = Math.floor((Date.now() - syncStart) / 1000);
        setSyncStatusMessage("info", `Diagram sync running for ${elapsedSec}s...`);
      }, SYNC_HEARTBEAT_MS);
    }, SYNC_SLOW_HINT_MS);
    const nextDocument: DiagramDocument = {
      modelName: doc.modelName,
      components: mergedComponents,
      connections,
      graphical: {
        layout: Object.keys(layout).length > 0 ? layout : undefined,
        diagramAnnotation: doc.graphical.diagramAnnotation,
        iconAnnotation: doc.graphical.iconAnnotation,
      },
    };
    applyGraphicalDocumentEdits(
      sourceRef.current,
      nextDocument,
      projectDirRef.current,
      filePathRef.current,
    )
      .then(({ newSource, warning }) => {
        if (activeSyncTokenRef.current !== syncToken) return;
        lastSelfEmittedSourceRef.current = newSource;
        onContentChangeRef.current?.(newSource);
        if (warning) {
          setMessages((prev) =>
            [{ severity: "warning", text: warning } as GraphicalMessage, ...prev].slice(0, 20),
          );
        }
        if (showedSlowStatus) {
          const elapsedSec = Math.max(1, Math.round((Date.now() - syncStart) / 1000));
          setSyncStatusMessage("info", `Diagram sync completed in ${elapsedSec}s.`);
        }
      })
      .catch((syncError) => {
        if (activeSyncTokenRef.current !== syncToken) return;
        lastAppliedRef.current = previousKey;
        clearSyncStatusMessage();
        setMessages((prev) =>
          [{ severity: "error", text: `Sync failed: ${String(syncError)}` } as GraphicalMessage, ...prev].slice(
            0,
            20,
          ),
        );
      })
      .finally(() => {
        if (activeSyncTokenRef.current !== syncToken) return;
        clearSyncTimers();
        // Keep completion message briefly; remove running status text only.
        setMessages((prev) =>
          prev.filter((m) => !m.text.startsWith(`${SYNC_STATUS_PREFIX}Diagram sync is still running`) && !m.text.includes("running for")),
        );
      });
  }, [clearSyncStatusMessage, clearSyncTimers, readOnly, session, setSyncStatusMessage]);

  const onRefreshDiagram = useCallback(
    (data: DiagramDocument) => {
      session.applyConflictRefresh(data);
      setSelectedGraphicPath(null);
      const nl = diagramToNodes(documentToDiagram(data), handleDoubleClickRef.current);
      lastAppliedRef.current = buildDiagramSyncKey(nl.nodes, nl.links, data.graphical);
    },
    [session],
  );

  const syncContentFingerprint = useMemo(() => {
    void sessionRevision;
    const doc = session.getDocument();
    if (!doc) return "";
    const { nodes, links } = session.getNodesLinksForCanvas(handleDoubleClickRef.current);
    return buildDiagramSyncKey(nodes, links, doc.graphical);
  }, [session, sessionRevision]);

  const selectedComponent = useMemo(() => {
    void sessionRevision;
    return session.getSelectedComponentForPanel(handleDoubleClickRef.current);
  }, [session, sessionRevision]);

  const updateSelectedParam = useCallback(
    (name: string, value: string) => {
      const sel = session.getStructureSelectionId();
      if (!sel) return;
      session.applyComponentParam(sel, name, value);
    },
    [session],
  );

  const updateSelectedPlacement = useCallback(
    (patch: { x?: number; y?: number; rotation?: number }) => {
      const sel = session.getStructureSelectionId();
      if (!sel) return;
      session.applyComponentPlacement(sel, patch);
    },
    [session],
  );

  const handleCommitDiagramCoordinateSystem = useCallback(
    (patch: import("./diagramGraphicTypes").CoordinateSystem) => {
      session.setCoordinateSystemForMode("diagram", patch);
    },
    [session],
  );

  const handleUpdateDeclaredType = useCallback(
    (typeName: string) => {
      const sel = session.getStructureSelectionId();
      if (!sel) return;
      session.applyComponentDeclaredType(sel, typeName);
    },
    [session],
  );

  const handleUpdateComponentFlags = useCallback(
    (patch: { condition?: string | null; visible?: boolean | null }) => {
      const sel = session.getStructureSelectionId();
      if (!sel) return;
      session.applyComponentAnnotationFlags(sel, patch);
    },
    [session],
  );

  const navigationTrail = useMemo(() => {
    const parts = [relativeFilePath?.replace(/\\/g, "/"), diagram?.modelName].filter(Boolean) as string[];
    return parts.length ? parts.join(" > ") : null;
  }, [relativeFilePath, diagram?.modelName]);

  const handleExportSvg = useCallback(() => {
    const annotation = diagram?.iconAnnotation;
    if (!annotation) return;
    downloadSvg(annotation, "icon-export.svg", { width: 800, height: 600, backgroundColor: "#ffffff" });
  }, [diagram?.iconAnnotation]);

  const handleExportPng = useCallback(async () => {
    const annotation = diagram?.iconAnnotation;
    if (!annotation) return;
    try {
      await downloadPng(annotation, "icon-export.png", { width: 800, height: 600, backgroundColor: "#ffffff" });
    } catch (e) {
      setMessages((prev) =>
        [{ severity: "error", text: `${t("exportPngFailed")}: ${String(e)}` } as GraphicalMessage, ...prev].slice(
          0,
          20,
        ),
      );
    }
  }, [diagram?.iconAnnotation]);

  useEffect(() => {
    if (readOnly || !onContentChangeRef.current || !session.getDocument()) return;
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    if (syncContentFingerprint === lastAppliedRef.current) return;
    timerRef.current = setTimeout(syncToSource, DEBOUNCE_MS);
    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
        timerRef.current = null;
      }
    };
  }, [syncContentFingerprint, readOnly, syncToSource]);

  useEffect(() => () => {
    clearSyncTimers();
  }, [clearSyncTimers]);

  useEffect(() => {
    if (!simOverlay.isActive || !simOverlay.overlayData) {
      session.setSimOverlay(null);
      return;
    }
    const map: Record<string, Record<string, number>> = {};
    const { nodes } = session.getNodesLinksForCanvas(handleDoubleClickRef.current);
    for (const node of nodes) {
      const vals = simOverlay.getNodeValues(node.id);
      if (vals) map[node.id] = vals;
    }
    session.setSimOverlay(Object.keys(map).length ? map : null);
  }, [simOverlay.isActive, simOverlay.overlayData, session]);

  useEffect(() => {
    if (!focusSymbolQuery) return;
    const targetNode = focusSymbolQuery.split(".")[0];
    session.setStructurePointerSelection(targetNode, false);
  }, [focusSymbolQuery, session]);

  const canConnect = useCallback(
    (source: string, _sourcePort: string, target: string, _targetPort: string) => {
      if (readOnly || modeRef.current === "icon") return false;
      const { nodes } = session.getNodesLinksForCanvas(handleDoubleClickRef.current);
      const sourceNode = nodes.find((n) => n.id === source);
      const targetNode = nodes.find((n) => n.id === target);
      const sourceKind = sourceNode?.data?.connectorKind;
      const targetKind = targetNode?.data?.connectorKind;
      const signalPair =
        (sourceKind === "signal_output" && targetKind === "signal_input") ||
        (sourceKind === "signal_input" && targetKind === "signal_output");
      const sameKind = sourceKind && targetKind && sourceKind === targetKind;
      if (sourceKind && targetKind && !sameKind && !signalPair) {
        setMessages((prev) =>
          [
            { severity: "error", text: `Incompatible connectors: ${sourceKind} -> ${targetKind}` } as GraphicalMessage,
            ...prev,
          ].slice(0, 20),
        );
        return false;
      }
      setMessages((prev) => prev.filter((m) => !m.text.startsWith("Incompatible connectors")));
      return true;
    },
    [readOnly, session, sessionRevision],
  );

  const structureNodeCount = useMemo(() => {
    void sessionRevision;
    return session.getNodesLinksForCanvas(handleDoubleClickRef.current).nodes.length;
  }, [session, sessionRevision]);

  const structureSelectedElementIds = useMemo(() => {
    void sessionRevision;
    const LINK_ID = /^e-.+-\d+$/;
    return session.getStructureSelectionIds().filter((id) => !LINK_ID.test(id));
  }, [session, sessionRevision]);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full text-[var(--text-muted)]">{loadingMessage}</div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-2 text-[var(--text-muted)] p-4">
        <span>
          {loadTimedOut ?
            t("diagramLoadTimeout")
          : error.includes("File defines a function, not a model") ?
            t("diagramErrorNotModel")
          : t("diagramErrorParse")}
        </span>
        {!loadTimedOut && <span className="text-xs">{error}</span>}
      </div>
    );
  }

  if (!diagram) {
    return (
      <div className="flex items-center justify-center h-full text-[var(--text-muted)]">{t("diagramEmpty")}</div>
    );
  }

  return (
    <GraphicalCanvas
      modelName={diagram.modelName}
      navigationTrail={navigationTrail}
      projectDir={projectDir}
      mode={mode}
      readOnly={readOnly}
      onOpenDependencyGraphSettings={onOpenDependencyGraphSettings}
      dependencyGraphBehavior={dependencyGraphBehavior}
      annotation={mode === "icon" ? diagram.iconAnnotation : diagram.diagramAnnotation}
      graphics={activeGraphics}
      selectedGraphicPath={selectedGraphicPath}
      selectedComponent={mode === "icon" ? null : selectedComponent}
      conflictPending={Boolean(conflictPending)}
      messages={messages}
      onRefreshDiagram={conflictPending ? () => onRefreshDiagram(conflictPending) : undefined}
      onSelectGraphic={handleSelectGraphic}
      onUpdateGraphic={handleUpdateGraphic}
      onAddGraphic={handleAddGraphic}
      onDeleteGraphic={handleDeleteGraphic}
      onUpdateParam={updateSelectedParam}
      onUpdatePlacement={updateSelectedPlacement}
      onUpdateDeclaredType={mode === "diagram" ? handleUpdateDeclaredType : undefined}
      onUpdateComponentFlags={mode === "diagram" ? handleUpdateComponentFlags : undefined}
      onOpenType={handleDoubleClick}
      libraryRefreshToken={libraryRefreshToken}
      stepDebug={stepDebug}
      onStartDebug={handleStartDebug}
      source={source}
      onDrop={(event) => {
        if (readOnly || mode === "icon") return;
        const payload = decodeModelicaDragPayload(event.dataTransfer.getData(MODELICA_DRAG_TYPE));
        if (!payload) return;
        event.preventDefault();
        const rect = event.currentTarget.getBoundingClientRect();
        const position: LayoutPoint = {
          x: Math.max(0, event.clientX - rect.left),
          y: Math.max(0, event.clientY - rect.top),
        };
        const { nodes } = session.getNodesLinksForCanvas(handleDoubleClickRef.current);
        const existingIds = nodes.map((n) => n.id);
        const id = uniqueInstanceName(payload.displayName, existingIds);
        session.applyDropComponent({
          id,
          typeName: payload.typeName,
          libraryId: payload.libraryId,
          position,
        });
      }}
      onDragOver={(event) => {
        if (Array.from(event.dataTransfer.types as ArrayLike<string>).includes(MODELICA_DRAG_TYPE)) {
          event.preventDefault();
        }
      }}
    >
      {mode === "icon" ? (
        <div className="flex flex-col flex-1 min-h-0">
          <div className="panel-header-bar shrink-0 flex items-center gap-2 border-b border-[var(--border)] bg-[var(--bg-elevated)] p-2">
            {selectedGraphicIndices.length >= 2 && (
              <AlignmentToolbar
                selectedGraphics={selectedGraphicIndices
                  .map((i) => diagram?.iconAnnotation?.graphics[i])
                  .filter((g): g is GraphicItem => !!g)}
                selectedIndices={selectedGraphicIndices}
                onAlign={handleAlign}
                onDistribute={handleDistribute}
              />
            )}
            {selectedGraphicIndices.length >= 1 && (
              <MultiSelectToolbar
                selectedIndices={selectedGraphicIndices}
                graphics={activeGraphics}
                onGroup={readOnly ? undefined : handleGroupGraphics}
                onUngroup={readOnly ? undefined : handleUngroupGraphics}
                onRectangleToPolygon={readOnly ? undefined : handleRectangleToPolygon}
                onDuplicate={handleDuplicate}
                onDelete={handleDeleteSelected}
              />
            )}
            <div className="flex-1" />
            <div className="flex items-center gap-2 text-xs">
              <label className="flex items-center gap-1 cursor-pointer">
                <input
                  type="checkbox"
                  checked={showGrid}
                  onChange={(e) => setShowGrid(e.target.checked)}
                  className="rounded"
                />
                <span>{t("showGrid")}</span>
              </label>
              <label className="flex items-center gap-1 cursor-pointer">
                <input
                  type="checkbox"
                  checked={gridEnabled}
                  onChange={(e) => setGridEnabled(e.target.checked)}
                  className="rounded"
                />
                <span>{t("gridSnap")}</span>
              </label>
              <select
                value={gridSize}
                onChange={(e) => setGridSize(Number(e.target.value))}
                className="rounded bg-[var(--surface)] border border-[var(--border)] px-1 py-0.5"
              >
                <option value={5}>5px</option>
                <option value={10}>10px</option>
                <option value={20}>20px</option>
                <option value={50}>50px</option>
              </select>
            </div>
            <div className="flex items-center gap-1">
              <button
                type="button"
                className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10 px-2 py-1 text-xs"
                onClick={handleExportSvg}
                title={t("exportSvg")}
              >
                SVG
              </button>
              <button
                type="button"
                className="toolbar-icon-btn flex rounded items-center justify-center text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-white/10 px-2 py-1 text-xs"
                onClick={handleExportPng}
                title={t("exportPng")}
              >
                PNG
              </button>
            </div>
          </div>
          <div className="flex flex-1 min-h-0 flex-row">
            <LayerPanel
              graphics={activeGraphics}
              selectedIndices={selectedGraphicIndices}
              readOnly={readOnly}
              onSelectLayer={(i, add) => handleSelectGraphic([i], add)}
              onToggleVisibility={handleToggleLayerHidden}
              onToggleLock={handleToggleLayerLocked}
              onReorder={handleReorderGraphics}
            />
            <div className="flex-1 min-h-0 min-w-0">
              <IconEditorShell
                annotation={diagram.iconAnnotation ?? { graphics: [] }}
                selectedGraphicPath={selectedGraphicPath}
                readOnly={readOnly}
                gridEnabled={gridEnabled}
                gridSize={gridSize}
                showGrid={showGrid}
                onSelectGraphic={handleSelectGraphic}
                onUpdateGraphic={handleUpdateGraphic}
              />
            </div>
          </div>
        </div>
      ) : (
        <div className="flex flex-col flex-1 min-h-0">
          <DiagramToolbar
            readOnly={readOnly}
            hasNodes={structureNodeCount > 0}
            onAddGraphic={handleAddGraphic}
            onApplyLayout={handleApplyLayout}
            paperHandle={paperHandle}
            showMiniMap={showMiniMap}
            onToggleMiniMap={() => setShowMiniMap((v) => !v)}
            structureToolbar={{
              snapMode: structureSnapMode,
              onSnapMode: setStructureSnapMode,
              gridSize: structureGridSize,
              onGridSize: setStructureGridSize,
              onRotate90: () => session.applyRotateSelected90(),
              onFlipH: () => session.applyFlipSelected("horizontal"),
              onFlipV: () => session.applyFlipSelected("vertical"),
              pointerText: pointerLocal ? `${pointerLocal.x.toFixed(1)}, ${pointerLocal.y.toFixed(1)}` : null,
              selectedElementIds: structureSelectedElementIds,
            }}
          />
          <DiagramCoordinateStrip
            readOnly={readOnly}
            coordinateSystem={diagram.diagramAnnotation?.coordinateSystem}
            onCommit={handleCommitDiagramCoordinateSystem}
          />
          <div className="flex-1 min-h-0">
            <JointStructureEditor
              session={session}
              readOnly={readOnly}
              onDoubleClick={handleDoubleClick}
              onAltDoubleClick={onOpenTypeSource ?? handleDoubleClick}
              onPaperReady={setPaperHandle}
              showMiniMap={showMiniMap}
              snapToGrid={structureSnapMode !== "none"}
              snapMode={structureSnapMode}
              structureGridSize={structureGridSize}
              onPointerPaperLocal={setPointerLocal}
              canConnect={canConnect}
              onDiagramSelection={() => setSelectedGraphicPath(null)}
            />
          </div>
        </div>
      )}
    </GraphicalCanvas>
  );
}
