import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { t } from "../i18n";
import {
  type AnnotationPoint,
  type GraphicItem,
  type IconDiagramAnnotation,
  type LineAnnotation,
} from "./DiagramSvgRenderer";
import { applyGraphicalDocumentEdits, getGraphicalDocumentFromSource } from "../api/tauri";
import type { GraphicalDocumentModel } from "../types";
import { GraphicalCanvas, type GraphicalMessage } from "./GraphicalCanvas";
import { decodeModelicaDragPayload, MODELICA_DRAG_TYPE } from "./LibrariesBrowser";
import {
  DiagramEditorView,
  type DiagramNode,
  type DiagramLink,
} from "./DiagramEditorView";
import { IconEditorView } from "./IconEditorView";
import { useUndoRedo } from "../hooks/useUndoRedo";
import { useStepDebug } from "../hooks/useStepDebug";
import { useDiagramSimulation } from "../hooks/useDiagramSimulation";
import { applyDiagramLayout, type DiagramLayoutKind } from "../utils/diagramLayout";
import { DiagramToolbar } from "./diagram/DiagramToolbar";
import type { JointPaperHandle } from "../utils/jointUtils";

interface DiagramUndoSnapshot {
  nodes: DiagramNode[];
  links: DiagramLink[];
}

export interface LayoutPoint {
  x: number;
  y: number;
}

interface Transformation {
  origin?: AnnotationPoint;
  extent?: { p1: AnnotationPoint; p2: AnnotationPoint };
  rotation?: number;
}

interface PlacementData {
  transformation?: Transformation;
  iconTransformation?: Transformation;
  visible?: boolean;
}

interface ParamValue {
  name: string;
  value: string;
}

interface ComponentData {
  name: string;
  typeName: string;
  libraryId?: string;
  placement?: PlacementData;
  icon?: IconDiagramAnnotation;
  rotation?: number;
  origin?: AnnotationPoint;
  params?: ParamValue[];
  connectorKind?: string;
  isInput?: boolean;
  isOutput?: boolean;
}

interface ConnectionData {
  from: string;
  to: string;
  line?: LineAnnotation;
}

type DiagramDocument = GraphicalDocumentModel<IconDiagramAnnotation, ComponentData, ConnectionData>;

interface DiagramModel {
  modelName: string;
  components: ComponentData[];
  connections: ConnectionData[];
  layout?: Record<string, LayoutPoint>;
  diagramAnnotation?: IconDiagramAnnotation;
  iconAnnotation?: IconDiagramAnnotation;
}

const DEFAULT_HANDLE_ID = "__default__";

function pathToNodeAndHandle(path: string): { nodeId: string; handleId: string } {
  const dot = path.indexOf(".");
  if (dot < 0) return { nodeId: path, handleId: DEFAULT_HANDLE_ID };
  return { nodeId: path.slice(0, dot), handleId: path.slice(dot + 1) };
}

function nodeAndHandleToPath(nodeId: string, handleId: string): string {
  if (handleId === DEFAULT_HANDLE_ID || !handleId) return nodeId;
  return `${nodeId}.${handleId}`;
}

const GRID_GAP = 180;
const ROW_GAP = 120;

function documentToDiagram(document: DiagramDocument): DiagramModel {
  return {
    modelName: document.modelName,
    components: document.components,
    connections: document.connections,
    layout: document.graphical.layout,
    diagramAnnotation: document.graphical.diagramAnnotation,
    iconAnnotation: document.graphical.iconAnnotation,
  };
}

function diagramToNodes(
  diagram: DiagramModel,
  onDoubleClick?: (typeName: string, libraryId?: string) => void
): { nodes: DiagramNode[]; links: DiagramLink[] } {
  const portUsage: Record<string, Set<string>> = {};
  for (const c of diagram.connections) {
    const { nodeId: fromId, handleId: fromH } = pathToNodeAndHandle(c.from);
    const { nodeId: toId, handleId: toH } = pathToNodeAndHandle(c.to);
    portUsage[fromId] = portUsage[fromId] ?? new Set();
    portUsage[fromId].add(fromH);
    portUsage[toId] = portUsage[toId] ?? new Set();
    portUsage[toId].add(toH);
  }

  const hasIncoming = new Set(diagram.connections.map((c) => pathToNodeAndHandle(c.to).nodeId));
  const hasOutgoing = new Set(diagram.connections.map((c) => pathToNodeAndHandle(c.from).nodeId));

  const layout = diagram.layout ?? {};
  const nodes: DiagramNode[] = diagram.components.map((comp, i) => {
    let position: { x: number; y: number };
    if (layout[comp.name]) {
      position = { x: layout[comp.name].x, y: layout[comp.name].y };
    } else if (comp.placement?.transformation?.extent) {
      const ext = comp.placement.transformation.extent;
      const origin = comp.placement.transformation.origin ?? { x: 0, y: 0 };
      position = {
        x: (ext.p1.x + ext.p2.x) / 2 + origin.x + 200,
        y: 200 - ((ext.p1.y + ext.p2.y) / 2 + origin.y),
      };
    } else {
      position = { x: (i % 4) * GRID_GAP, y: Math.floor(i / 4) * ROW_GAP };
    }

    const ports = portUsage[comp.name] ? Array.from(portUsage[comp.name]) : [DEFAULT_HANDLE_ID];
    const incoming = hasIncoming.has(comp.name);
    const outgoing = hasOutgoing.has(comp.name);

    return {
      id: comp.name,
      position,
      data: {
        typeName: comp.typeName,
        libraryId: comp.libraryId,
        portHandles: ports,
        icon: comp.icon,
        rotation: comp.rotation,
        params: comp.params,
        connectorKind: comp.connectorKind,
        isInput: comp.isInput,
        isOutput: comp.isOutput,
        isSourceNode: !incoming && outgoing,
        isSinkNode: incoming && !outgoing,
        onDoubleClick,
      },
    };
  });

  const links: DiagramLink[] = diagram.connections.map((conn, i) => {
    const a = pathToNodeAndHandle(conn.from);
    const b = pathToNodeAndHandle(conn.to);
    return {
      id: `e-${conn.from}-${conn.to}-${i}`,
      source: a.nodeId,
      sourcePort: a.handleId,
      target: b.nodeId,
      targetPort: b.handleId,
      vertices: conn.line?.points,
    };
  });

  return { nodes, links };
}

function nodesToDiagram(
  nodes: DiagramNode[],
  links: DiagramLink[]
): {
  components: {
    name: string;
    typeName: string;
    libraryId?: string;
    params?: ParamValue[];
    isInput?: boolean;
    isOutput?: boolean;
  }[];
  connections: { from: string; to: string; line?: LineAnnotation }[];
  layout: Record<string, LayoutPoint>;
} {
  const components = nodes.map((n) => ({
    name: n.id,
    typeName: n.data?.typeName || "Block",
    libraryId: n.data?.libraryId as string | undefined,
    params: n.data?.params,
    isInput: Boolean(n.data?.isInput),
    isOutput: Boolean(n.data?.isOutput),
  }));
  const connections = links.map((l) => ({
    from: nodeAndHandleToPath(l.source, l.sourcePort),
    to: nodeAndHandleToPath(l.target, l.targetPort),
    line: l.vertices?.length ? { points: l.vertices } : undefined,
  }));
  const layout: Record<string, LayoutPoint> = {};
  nodes.forEach((n) => {
    layout[n.id] = { x: n.position.x, y: n.position.y };
  });
  return { components, connections, layout };
}

const DEBOUNCE_MS = 600;

function uniqueInstanceName(typeName: string, existingIds: string[]): string {
  const rawBase = typeName.split(".").pop() ?? typeName;
  const sanitized = rawBase.replace(/[^A-Za-z0-9_]/g, "");
  const base = sanitized.length > 0
    ? sanitized[0].toLowerCase() + sanitized.slice(1)
    : "c";
  const set = new Set(existingIds);
  if (!set.has(base)) return base;
  let i = 1;
  while (set.has(base + i)) i++;
  return base + i;
}

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
  const [document, setDocument] = useState<DiagramDocument | null>(null);
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

  const handleDoubleClick = useCallback(
    (typeName: string, libraryId?: string) => {
      onNavigateToType?.(typeName, libraryId);
    },
    [onNavigateToType]
  );

  const handleDoubleClickRef = useRef(handleDoubleClick);
  handleDoubleClickRef.current = handleDoubleClick;

  const diagram = useMemo(() => (document ? documentToDiagram(document) : null), [document]);
  const initial = useMemo(() => {
    const { nodes, links } = diagramToNodes(
      diagram ?? { modelName: "", components: [], connections: [] },
      handleDoubleClickRef.current
    );
    return { nodes, links };
  }, [!!diagram]);

  const [nodes, setNodes] = useState<DiagramNode[]>(initial.nodes);
  const [links, setLinks] = useState<DiagramLink[]>(initial.links);

  const nodesRef = useRef(nodes);
  nodesRef.current = nodes;
  const linksRef = useRef(links);
  linksRef.current = links;
  const documentRef = useRef(document);
  documentRef.current = document;
  const [selectedGraphicIndex, setSelectedGraphicIndex] = useState(-1);
  const [paperHandle, setPaperHandle] = useState<JointPaperHandle | null>(null);
  const [showMiniMap, setShowMiniMap] = useState(true);

  const undoRedo = useUndoRedo<DiagramUndoSnapshot>();
  const undoRedoSkip = useRef(false);

  const stepDebug = useStepDebug();
  const simOverlay = useDiagramSimulation(stepDebug);

  const handleStartDebug = useCallback(() => {
    stepDebug.startSession(sourceRef.current, diagram?.modelName, projectDirRef.current);
  }, [diagram?.modelName, stepDebug]);

  const handleApplyLayout = useCallback(
    (kind: DiagramLayoutKind) => {
      const currentNodes = nodesRef.current;
      const currentLinks = linksRef.current;
      if (currentNodes.length === 0) return;
      const nodeIds = currentNodes.map((n) => n.id);
      const edges = currentLinks.map((l) => ({ source: l.source, target: l.target }));
      const positions = applyDiagramLayout(kind, { nodeIds, edges });
      undoRedoSkip.current = true;
      setNodes((prev) =>
        prev.map((node) => ({
          ...node,
          position: positions[node.id] ?? node.position,
        }))
      );
    },
    []
  );

  useEffect(() => {
    if (undoRedoSkip.current) {
      undoRedoSkip.current = false;
      return;
    }
    if (nodes.length > 0 || links.length > 0) {
      undoRedo.push({
        nodes: JSON.parse(JSON.stringify(nodes)),
        links: JSON.parse(JSON.stringify(links)),
      });
    }
  }, [nodes, links]);

  useEffect(() => {
    if (readOnly) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      if (e.key === "z" && !e.shiftKey && undoRedo.canUndo) {
        e.preventDefault();
        e.stopPropagation();
        const snapshot = undoRedo.undo();
        if (snapshot) {
          undoRedoSkip.current = true;
          setNodes(snapshot.nodes);
          setLinks(snapshot.links);
        }
      } else if (
        ((e.key === "z" && e.shiftKey) || e.key === "y") &&
        undoRedo.canRedo
      ) {
        e.preventDefault();
        e.stopPropagation();
        const snapshot = undoRedo.redo();
        if (snapshot) {
          undoRedoSkip.current = true;
          setNodes(snapshot.nodes);
          setLinks(snapshot.links);
        }
      }
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [readOnly, undoRedo.canUndo, undoRedo.canRedo]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setConflictPending(null);
    getGraphicalDocumentFromSource<IconDiagramAnnotation, ComponentData, ConnectionData>(
      source,
      projectDir,
      relativeFilePath,
    )
      .then((data) => {
        if (cancelled) return;
        const current = nodesToDiagram(nodesRef.current, linksRef.current);
        const hasCurrentState = current.components.length > 0 || current.connections.length > 0;
        const sameComponents =
          current.components.length === data.components.length &&
          current.components.every(
            (c, i) =>
              data.components[i] &&
              c.name === data.components[i].name &&
              c.typeName === data.components[i].typeName
          );
        const sameConnections =
          current.connections.length === data.connections.length &&
          current.connections.every(
            (c, i) =>
              data.connections[i] &&
              c.from === data.connections[i].from &&
              c.to === data.connections[i].to
          );
        const inSync = sameComponents && sameConnections;
        if (!hasCurrentState || inSync) {
          setDocument(data);
          setSelectedGraphicIndex(-1);
          const { nodes: n, links: l } = diagramToNodes(documentToDiagram(data), handleDoubleClickRef.current);
          setNodes(n);
          setLinks(l);
          setConflictPending(null);
          lastAppliedRef.current = JSON.stringify({
            components: data.components.map((c) => ({ name: c.name, typeName: c.typeName })),
            connections: data.connections.map((c) => ({ from: c.from, to: c.to })),
            layout: data.graphical.layout ?? {},
          });
        } else {
          setConflictPending(data);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(String(err));
          setDocument(null);
          setNodes([]);
          setLinks([]);
          setConflictPending(null);
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [source]);

  const handleNodePositionChange = useCallback(
    (id: string, position: { x: number; y: number }) => {
      setNodes((prev) =>
        prev.map((n) => (n.id === id ? { ...n, position } : n))
      );
    },
    []
  );

  const handleConnect = useCallback(
    (source: string, sourcePort: string, target: string, targetPort: string) => {
      if (readOnly || mode === "icon") return;
      const sourceNode = nodesRef.current.find((n) => n.id === source);
      const targetNode = nodesRef.current.find((n) => n.id === target);
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
          ].slice(0, 20)
        );
        return;
      }
      setMessages((prev) => prev.filter((m) => !m.text.startsWith("Incompatible connectors")));
      const newLink: DiagramLink = {
        id: `e-${nodeAndHandleToPath(source, sourcePort)}-${nodeAndHandleToPath(target, targetPort)}-${Date.now()}`,
        source,
        sourcePort,
        target,
        targetPort,
      };
      setLinks((prev) => [...prev, newLink]);
    },
    [mode, readOnly]
  );

  const handleSelectNode = useCallback(
    (id: string | null) => {
      setNodes((prev) =>
        prev.map((n) => ({
          ...n,
          selected: id != null && n.id === id,
        }))
      );
      setSelectedGraphicIndex(-1);
    },
    []
  );

  const handleDeleteElements = useCallback(
    (nodeIds: string[], linkIds: string[]) => {
      if (readOnly) return;
      const nodeIdSet = new Set(nodeIds);
      const linkIdSet = new Set(linkIds);
      setNodes((prev) => prev.filter((n) => !nodeIdSet.has(n.id)));
      setLinks((prev) =>
        prev.filter(
          (l) =>
            !linkIdSet.has(l.id) &&
            !nodeIdSet.has(l.source) &&
            !nodeIdSet.has(l.target)
        )
      );
    },
    [readOnly]
  );

  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastAppliedRef = useRef<string>("");

  const syncToSource = useCallback(() => {
    if (readOnly || !onContentChangeRef.current) return;
    const { components, connections, layout } = nodesToDiagram(nodesRef.current, linksRef.current);
    const key = JSON.stringify({ components, connections, layout });
    if (key === lastAppliedRef.current) return;
    lastAppliedRef.current = key;
    const nextDocument: DiagramDocument = {
      modelName: documentRef.current?.modelName ?? diagram?.modelName ?? "",
      components,
      connections,
      graphical: {
        layout: Object.keys(layout).length > 0 ? layout : undefined,
        diagramAnnotation: documentRef.current?.graphical.diagramAnnotation,
        iconAnnotation: documentRef.current?.graphical.iconAnnotation,
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
        setMessages((prev) =>
          [
            { severity: "error", text: `Sync failed: ${String(syncError)}` } as GraphicalMessage,
            ...prev,
          ].slice(0, 20)
        );
      });
  }, [diagram?.modelName, readOnly]);

  const onRefreshDiagram = useCallback(
    (data: DiagramDocument) => {
      setDocument(data);
      setSelectedGraphicIndex(-1);
      const { nodes: n, links: l } = diagramToNodes(documentToDiagram(data), handleDoubleClickRef.current);
      setNodes(n);
      setLinks(l);
      setConflictPending(null);
      lastAppliedRef.current = JSON.stringify({
        components: data.components.map((c) => ({ name: c.name, typeName: c.typeName })),
        connections: data.connections.map((c) => ({ from: c.from, to: c.to })),
        layout: data.graphical.layout ?? {},
      });
    },
    []
  );

  const activeGraphics = mode === "icon"
    ? (diagram?.iconAnnotation?.graphics ?? [])
    : (diagram?.diagramAnnotation?.graphics ?? []);

  const updateActiveAnnotation = useCallback((graphics: GraphicItem[]) => {
    setDocument((prev) => {
      if (!prev) return prev;
      const next = {
        ...prev,
        graphical: {
          ...prev.graphical,
        },
      };
      if (mode === "icon") {
        next.graphical.iconAnnotation = {
          coordinateSystem: prev.graphical.iconAnnotation?.coordinateSystem,
          graphics,
        };
      } else {
        next.graphical.diagramAnnotation = {
          coordinateSystem: prev.graphical.diagramAnnotation?.coordinateSystem,
          graphics,
        };
      }
      return next;
    });
  }, [mode]);

  const selectedNode = useMemo(
    () => nodes.find((node) => node.selected),
    [nodes]
  );

  const selectedComponent = useMemo(() => {
    if (!selectedNode) return null;
    return {
      name: selectedNode.id,
      typeName: selectedNode.data?.typeName ?? "",
      libraryId: selectedNode.data?.libraryId as string | undefined,
      params: selectedNode.data?.params ?? [],
      placement: {
        transformation: {
          origin: { x: selectedNode.position.x, y: selectedNode.position.y },
          rotation: selectedNode.data?.rotation,
        },
      },
    };
  }, [selectedNode]);

  const updateSelectedParam = useCallback((name: string, value: string) => {
    if (!selectedNode) return;
    setNodes((prev) =>
      prev.map((node) => {
        if (node.id !== selectedNode.id) return node;
        const params = [...(node.data?.params ?? [])];
        const index = params.findIndex((item) => item.name === name);
        if (index >= 0) {
          params[index] = { ...params[index], value };
        } else {
          params.push({ name, value });
        }
        return { ...node, data: { ...node.data, params } };
      })
    );
  }, [selectedNode]);

  const updateSelectedPlacement = useCallback((patch: { x?: number; y?: number; rotation?: number }) => {
    if (!selectedNode) return;
    setNodes((prev) =>
      prev.map((node) =>
        node.id !== selectedNode.id
          ? node
          : {
              ...node,
              position: {
                x: patch.x ?? node.position.x,
                y: patch.y ?? node.position.y,
              },
              data: {
                ...node.data,
                rotation: patch.rotation ?? node.data?.rotation,
              },
            }
      )
    );
  }, [selectedNode]);

  const handleUpdateGraphic = useCallback((index: number, next: GraphicItem) => {
    const graphics = [...activeGraphics];
    graphics[index] = next;
    updateActiveAnnotation(graphics);
  }, [activeGraphics, updateActiveAnnotation]);

  const handleAddGraphic = useCallback((graphic: GraphicItem) => {
    const graphics = [...activeGraphics, graphic];
    updateActiveAnnotation(graphics);
    setSelectedGraphicIndex(graphics.length - 1);
  }, [activeGraphics, updateActiveAnnotation]);

  const handleDeleteGraphic = useCallback((index: number) => {
    const graphics = activeGraphics.filter((_, itemIndex) => itemIndex !== index);
    updateActiveAnnotation(graphics);
    setSelectedGraphicIndex(-1);
  }, [activeGraphics, updateActiveAnnotation]);

  useEffect(() => {
    if (readOnly || !onContentChange || !diagram) return;
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(syncToSource, DEBOUNCE_MS);
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [nodes, links, diagram?.diagramAnnotation, diagram?.iconAnnotation, readOnly, onContentChange, diagram, syncToSource]);

  useEffect(() => {
    if (!simOverlay.isActive || !simOverlay.overlayData) return;
    setNodes((prev) =>
      prev.map((node) => {
        const vals = simOverlay.getNodeValues(node.id);
        if (!vals) return node;
        return {
          ...node,
          data: {
            ...node.data,
            simValues: vals ?? undefined,
          },
        };
      }),
    );
  }, [simOverlay.isActive, simOverlay.overlayData]);

  useEffect(() => {
    if (!focusSymbolQuery) return;
    const targetNode = focusSymbolQuery.split(".")[0];
    setNodes((prev) =>
      prev.map((node) => ({
        ...node,
        selected: node.id === targetNode,
      })),
    );
  }, [focusSymbolQuery]);

  const nodesWithFlowRole = useMemo(() => {
    const hasIncoming = new Set(links.map((l) => l.target));
    const hasOutgoing = new Set(links.map((l) => l.source));
    return nodes.map((node) => {
      const incoming = hasIncoming.has(node.id);
      const outgoing = hasOutgoing.has(node.id);
      return {
        ...node,
        data: {
          ...node.data,
          isSourceNode: !incoming && outgoing,
          isSinkNode: incoming && !outgoing,
        },
      };
    });
  }, [nodes, links]);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full text-[var(--text-muted)]">
        {t("diagramLoading")}
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-2 text-[var(--text-muted)] p-4">
        <span>{error.includes("File defines a function, not a model") ? t("diagramErrorNotModel") : t("diagramErrorParse")}</span>
        <span className="text-xs">{error}</span>
      </div>
    );
  }

  if (!diagram) {
    return (
      <div className="flex items-center justify-center h-full text-[var(--text-muted)]">
        {t("diagramEmpty")}
      </div>
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
          const position = {
            x: Math.max(0, event.clientX - rect.left),
            y: Math.max(0, event.clientY - rect.top),
          };
          setNodes((nds) => {
            const existingIds = nds.map((n) => n.id);
            const id = uniqueInstanceName(payload.displayName, existingIds);
            return nds.concat({
              id,
              position,
              data: {
                typeName: payload.typeName,
                libraryId: payload.libraryId,
                portHandles: [DEFAULT_HANDLE_ID],
                params: [],
              },
            });
          });
        }}
        onDragOver={(event) => {
          if (Array.from(event.dataTransfer.types as ArrayLike<string>).includes(MODELICA_DRAG_TYPE)) {
            event.preventDefault();
          }
        }}
      >
        {mode === "icon" ? (
          <IconEditorView
            annotation={diagram.iconAnnotation ?? { graphics: [] }}
            selectedGraphicIndex={selectedGraphicIndex}
            readOnly={readOnly}
            onSelectGraphic={setSelectedGraphicIndex}
            onUpdateGraphic={handleUpdateGraphic}
          />
        ) : (
          <div className="flex flex-col flex-1 min-h-0">
            <DiagramToolbar
              readOnly={readOnly}
              hasNodes={nodes.length > 0}
              onAddGraphic={handleAddGraphic}
              onApplyLayout={handleApplyLayout}
              paperHandle={paperHandle}
              showMiniMap={showMiniMap}
              onToggleMiniMap={() => setShowMiniMap((v) => !v)}
            />
            <div className="flex-1 min-h-0">
              <DiagramEditorView
                nodes={nodesWithFlowRole}
                links={links}
                readOnly={readOnly}
                onNodePositionChange={handleNodePositionChange}
                onConnect={handleConnect}
                onDeleteElements={handleDeleteElements}
                onSelectNode={handleSelectNode}
                onPaperReady={setPaperHandle}
                showMiniMap={showMiniMap}
              />
            </div>
          </div>
        )}
      </GraphicalCanvas>
  );
}
