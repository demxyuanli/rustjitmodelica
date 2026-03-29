import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { t } from "../i18n";
import type { GraphicItem, IconDiagramAnnotation } from "./DiagramSvgRenderer";
import { applyGraphicalDocumentEdits, getGraphicalDocumentFromSource } from "../api/tauri";
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
import { createStructureGraphSession } from "../structureEditor/session";
import { JointStructureEditor } from "../structureEditor/JointStructureEditor";
import { IconEditorShell } from "../structureEditor/IconEditorShell";
import { useStepDebug } from "../hooks/useStepDebug";
import { useDiagramSimulation } from "../hooks/useDiagramSimulation";
import { applyDiagramLayout, type DiagramLayoutKind } from "../utils/diagramLayout";
import { DiagramToolbar } from "./diagram/DiagramToolbar";
import { AlignmentToolbar, alignGraphics, distributeGraphics } from "./diagram/AlignmentToolbar";
import { MultiSelectToolbar } from "./diagram/MultiSelectToolbar";
import { downloadSvg, downloadPng } from "../utils/graphicExport";
import { duplicateGraphics, deleteGraphics } from "../utils/graphicGroup";
import type { JointPaperHandle } from "../utils/jointUtils";

export type { LayoutPoint } from "../structureEditor/types";

export interface DiagramViewProps {
  source: string;
  projectDir: string | null;
  relativeFilePath?: string | null;
  onContentChange?: (newSource: string) => void;
  readOnly?: boolean;
  onNavigateToType?: (typeName: string, libraryId?: string) => void;
  mode?: "diagram" | "icon";
  focusSymbolQuery?: string | null;
  libraryRefreshToken?: number;
}

export function DiagramView({
  source,
  projectDir,
  relativeFilePath,
  onContentChange,
  readOnly = false,
  onNavigateToType,
  mode = "diagram",
  focusSymbolQuery = null,
  libraryRefreshToken = 0,
}: DiagramViewProps) {
  const sessionRef = useRef<ReturnType<typeof createStructureGraphSession> | null>(null);
  if (!sessionRef.current) sessionRef.current = createStructureGraphSession();
  const session = sessionRef.current;

  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [conflictPending, setConflictPending] = useState<DiagramDocument | null>(null);
  const [messages, setMessages] = useState<GraphicalMessage[]>([]);

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

  const [selectedGraphicIndex, setSelectedGraphicIndex] = useState(-1);
  const [selectedGraphicIndices, setSelectedGraphicIndices] = useState<number[]>([]);
  const [paperHandle, setPaperHandle] = useState<JointPaperHandle | null>(null);
  const [showMiniMap, setShowMiniMap] = useState(true);
  const [gridEnabled, setGridEnabled] = useState(true);
  const [gridSize, setGridSize] = useState(10);
  const [showGrid, setShowGrid] = useState(true);

  const loadGraphGenerationRef = useRef(0);

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
    let cancelled = false;
    const gen = ++loadGraphGenerationRef.current;
    setLoading(true);
    setError(null);
    setConflictPending(null);
    getGraphicalDocumentFromSource<IconDiagramAnnotation, ComponentData, ConnectionData>(
      source,
      projectDir,
      relativeFilePath,
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
          setSelectedGraphicIndex(-1);
          setConflictPending(null);
          const nlLoaded = diagramToNodes(documentToDiagram(data), handleDoubleClickRef.current);
          lastAppliedRef.current = buildDiagramSyncKey(nlLoaded.nodes, nlLoaded.links, data.graphical);
        } else {
          setConflictPending(data);
        }
      })
      .catch((err) => {
        if (cancelled || gen !== loadGraphGenerationRef.current) return;
        setError(String(err));
        setConflictPending(null);
        session.clearDocument();
      })
      .finally(() => {
        if (!cancelled && gen === loadGraphGenerationRef.current) {
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [source, session]);

  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastAppliedRef = useRef<string>("");

  const syncToSource = useCallback(() => {
    if (readOnly || !onContentChangeRef.current) return;
    const doc = session.getDocument();
    if (!doc) return;
    const { nodes, links } = session.getNodesLinksForCanvas(handleDoubleClickRef.current);
    const key = buildDiagramSyncKey(nodes, links, doc.graphical);
    if (key === lastAppliedRef.current) return;
    const { components, connections, layout } = nodesToDiagram(nodes, links);
    const previousKey = lastAppliedRef.current;
    lastAppliedRef.current = key;
    const nextDocument: DiagramDocument = {
      modelName: doc.modelName,
      components,
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
      .then(({ newSource }) => {
        onContentChangeRef.current?.(newSource);
      })
      .catch((syncError) => {
        lastAppliedRef.current = previousKey;
        setMessages((prev) =>
          [{ severity: "error", text: `Sync failed: ${String(syncError)}` } as GraphicalMessage, ...prev].slice(
            0,
            20,
          ),
        );
      });
  }, [readOnly, session]);

  const onRefreshDiagram = useCallback(
    (data: DiagramDocument) => {
      session.applyConflictRefresh(data);
      setSelectedGraphicIndex(-1);
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

  const activeGraphics =
    mode === "icon" ? (diagram?.iconAnnotation?.graphics ?? []) : (diagram?.diagramAnnotation?.graphics ?? []);

  const setGraphicsForActiveMode = useCallback(
    (graphics: GraphicItem[]) => {
      session.setGraphicsForMode(modeRef.current, graphics);
    },
    [session],
  );

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

  const handleUpdateGraphic = useCallback(
    (index: number, next: GraphicItem) => {
      const graphics = [...activeGraphics];
      graphics[index] = next;
      setGraphicsForActiveMode(graphics);
    },
    [activeGraphics, setGraphicsForActiveMode],
  );

  const handleAddGraphic = useCallback(
    (graphic: GraphicItem) => {
      const graphics = [...activeGraphics, graphic];
      setGraphicsForActiveMode(graphics);
      setSelectedGraphicIndex(graphics.length - 1);
    },
    [activeGraphics, setGraphicsForActiveMode],
  );

  const handleDeleteGraphic = useCallback(
    (index: number) => {
      const graphics = activeGraphics.filter((_, itemIndex) => itemIndex !== index);
      setGraphicsForActiveMode(graphics);
      setSelectedGraphicIndex(-1);
    },
    [activeGraphics, setGraphicsForActiveMode],
  );

  const handleAlign = useCallback(
    (alignment: "left" | "center" | "right" | "top" | "middle" | "bottom") => {
      if (selectedGraphicIndices.length < 2) return;
      const updated = alignGraphics(activeGraphics, selectedGraphicIndices, alignment);
      setGraphicsForActiveMode(updated);
      setSelectedGraphicIndices([]);
    },
    [activeGraphics, selectedGraphicIndices, setGraphicsForActiveMode],
  );

  const handleDistribute = useCallback(
    (distribution: "horizontal" | "vertical") => {
      if (selectedGraphicIndices.length < 3) return;
      const updated = distributeGraphics(activeGraphics, selectedGraphicIndices, distribution);
      setGraphicsForActiveMode(updated);
      setSelectedGraphicIndices([]);
    },
    [activeGraphics, selectedGraphicIndices, setGraphicsForActiveMode],
  );

  const handleDuplicate = useCallback(() => {
    if (selectedGraphicIndices.length === 0) return;
    const { updatedGraphics } = duplicateGraphics(activeGraphics, selectedGraphicIndices);
    setGraphicsForActiveMode(updatedGraphics);
    setSelectedGraphicIndices([]);
  }, [activeGraphics, selectedGraphicIndices, setGraphicsForActiveMode]);

  const handleDeleteSelected = useCallback(() => {
    if (selectedGraphicIndices.length === 0) return;
    const updated = deleteGraphics(activeGraphics, selectedGraphicIndices);
    setGraphicsForActiveMode(updated);
    setSelectedGraphicIndices([]);
  }, [activeGraphics, selectedGraphicIndices, setGraphicsForActiveMode]);

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

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full text-[var(--text-muted)]">{t("diagramLoading")}</div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-2 text-[var(--text-muted)] p-4">
        <span>
          {error.includes("File defines a function, not a model") ? t("diagramErrorNotModel") : t("diagramErrorParse")}
        </span>
        <span className="text-xs">{error}</span>
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
      projectDir={projectDir}
      mode={mode}
      readOnly={readOnly}
      annotation={mode === "icon" ? diagram.iconAnnotation : diagram.diagramAnnotation}
      graphics={activeGraphics}
      selectedGraphicIndex={selectedGraphicIndex}
      selectedComponent={mode === "icon" ? null : selectedComponent}
      conflictPending={Boolean(conflictPending)}
      messages={messages}
      onRefreshDiagram={conflictPending ? () => onRefreshDiagram(conflictPending) : undefined}
      onSelectGraphic={setSelectedGraphicIndex}
      onUpdateGraphic={handleUpdateGraphic}
      onAddGraphic={handleAddGraphic}
      onDeleteGraphic={handleDeleteGraphic}
      onUpdateParam={updateSelectedParam}
      onUpdatePlacement={updateSelectedPlacement}
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
          <IconEditorShell
            annotation={diagram.iconAnnotation ?? { graphics: [] }}
            selectedGraphicIndex={selectedGraphicIndex}
            readOnly={readOnly}
            gridEnabled={gridEnabled}
            gridSize={gridSize}
            showGrid={showGrid}
            onSelectGraphic={(index) => {
              setSelectedGraphicIndex(index);
              if (index >= 0) {
                setSelectedGraphicIndices((prev) =>
                  prev.length === 1 && prev[0] === index ? prev : [index],
                );
              } else {
                setSelectedGraphicIndices([]);
              }
            }}
            onUpdateGraphic={handleUpdateGraphic}
          />
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
          />
          <div className="flex-1 min-h-0">
            <JointStructureEditor
              session={session}
              readOnly={readOnly}
              onDoubleClick={handleDoubleClick}
              onPaperReady={setPaperHandle}
              showMiniMap={showMiniMap}
              snapToGrid
              canConnect={canConnect}
              onDiagramSelection={() => setSelectedGraphicIndex(-1)}
            />
          </div>
        </div>
      )}
    </GraphicalCanvas>
  );
}
