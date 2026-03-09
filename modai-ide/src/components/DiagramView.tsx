import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ReactFlow,
  ReactFlowProvider,
  useNodesState,
  useEdgesState,
  addEdge,
  useReactFlow,
  type Connection,
  type Edge,
  type Node,
  type NodeProps,
  Handle,
  Position,
  Background,
  Controls,
  MiniMap,
  Panel,
  type EdgeProps,
  BaseEdge,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import {
  type AnnotationPoint,
  type IconDiagramAnnotation,
  type LineAnnotation,
  IconSvg,
  connectorHandleStyle,
  DEFAULT_ICON_SIZE,
} from "./DiagramSvgRenderer";

// ---------------------------------------------------------------------------
// Types matching enriched backend DiagramModel
// ---------------------------------------------------------------------------

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

export interface DiagramModel {
  modelName: string;
  components: ComponentData[];
  connections: ConnectionData[];
  layout?: Record<string, LayoutPoint>;
  diagramAnnotation?: IconDiagramAnnotation;
  iconAnnotation?: IconDiagramAnnotation;
}

// ---------------------------------------------------------------------------
// Path utilities
// ---------------------------------------------------------------------------

function pathToNodeAndHandle(path: string): { nodeId: string; handleId: string } {
  const dot = path.indexOf(".");
  if (dot < 0) return { nodeId: path, handleId: "p" };
  return { nodeId: path.slice(0, dot), handleId: path.slice(dot + 1) };
}

function nodeAndHandleToPath(nodeId: string, handleId: string): string {
  if (handleId === "p" || !handleId) return nodeId;
  return `${nodeId}.${handleId}`;
}

// ---------------------------------------------------------------------------
// Component Node (memoized)
// ---------------------------------------------------------------------------

type DiagramNodeData = {
  typeName: string;
  portHandles: string[];
  icon?: IconDiagramAnnotation;
  rotation?: number;
  params?: ParamValue[];
  connectorKind?: string;
  onDoubleClick?: (typeName: string) => void;
};

const ComponentNode = React.memo(function ComponentNode(props: NodeProps<Node<DiagramNodeData>>) {
  const { id, data, selected } = props;
  const safeData = data ?? { typeName: "Block", portHandles: ["p"] };
  const ports = safeData.portHandles?.length ? safeData.portHandles : ["p"];
  const hasIcon = safeData.icon && safeData.icon.graphics && safeData.icon.graphics.length > 0;
  const iconSize = DEFAULT_ICON_SIZE;

  const paramStr = safeData.params
    ?.filter((p) => p.value)
    .map((p) => (p.name ? `${p.name}=${p.value}` : p.value))
    .join(", ");

  const portStyles = useMemo(
    () =>
      ports.map((_port: string, i: number) => {
        const pct = ports.length === 1 ? 50 : 20 + (i * 60) / Math.max(1, ports.length - 1);
        const base = connectorHandleStyle(safeData.connectorKind, "right");
        const baseL = connectorHandleStyle(safeData.connectorKind, "left");
        return {
          pct,
          right: { ...base, top: `${pct}%`, transform: "translateY(-50%)" } as React.CSSProperties,
          left: { ...baseL, top: `${pct}%`, transform: "translateY(-50%)" } as React.CSSProperties,
        };
      }),
    [ports.length, safeData.connectorKind]
  );

  return (
    <div
      className={`rounded border bg-[var(--bg-elevated)] border-[var(--border)] relative ${selected ? "ring-2 ring-primary" : ""}`}
      style={{ minWidth: hasIcon ? iconSize + 16 : 80, padding: hasIcon ? 4 : "8px 12px" }}
      onDoubleClick={() => safeData.onDoubleClick?.(safeData.typeName)}
    >
      {hasIcon ? (
        <div className="flex flex-col items-center gap-0.5">
          <IconSvg
            icon={safeData.icon!}
            instanceName={id}
            rotation={safeData.rotation}
            size={iconSize}
          />
          <div className="text-[9px] font-medium text-[var(--text)] text-center leading-tight truncate max-w-[60px]">
            {id}
          </div>
          {paramStr && (
            <div className="text-[8px] text-[var(--text-muted)] text-center truncate max-w-[70px]">
              {paramStr}
            </div>
          )}
        </div>
      ) : (
        <>
          <div className="text-xs font-medium text-[var(--text)]">{id}</div>
          <div className="text-[10px] text-[var(--text-muted)] truncate">{safeData.typeName}</div>
          {paramStr && (
            <div className="text-[9px] text-[var(--text-muted)] mt-0.5 truncate max-w-[100px]">
              {paramStr}
            </div>
          )}
        </>
      )}
      {ports.map((port: string, i: number) => (
        <React.Fragment key={port}>
          <Handle
            type="source"
            id={port}
            position={Position.Right}
            style={portStyles[i].right}
          />
          <Handle
            type="target"
            id={port}
            position={Position.Left}
            style={portStyles[i].left}
          />
        </React.Fragment>
      ))}
    </div>
  );
});

// ---------------------------------------------------------------------------
// Polyline Edge (memoized)
// ---------------------------------------------------------------------------

const PolylineEdge = React.memo(function PolylineEdge(props: EdgeProps) {
  const { sourceX, sourceY, targetX, targetY, style, markerEnd } = props;
  const edgeData = (props as any).data as { linePoints?: AnnotationPoint[] } | undefined;

  if (edgeData?.linePoints && edgeData.linePoints.length >= 2) {
    const pts = edgeData.linePoints;
    const d = pts.map((p, i) => `${i === 0 ? "M" : "L"} ${p.x} ${p.y}`).join(" ");
    return <BaseEdge path={d} style={style} markerEnd={markerEnd} />;
  }

  const midX = (sourceX + targetX) / 2;
  const d = `M ${sourceX} ${sourceY} C ${midX} ${sourceY}, ${midX} ${targetY}, ${targetX} ${targetY}`;
  return <BaseEdge path={d} style={style} markerEnd={markerEnd} />;
});

const nodeTypes = { component: ComponentNode as React.ComponentType<NodeProps<Node<DiagramNodeData>>> };
const edgeTypes = { polyline: PolylineEdge };

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

const GRID_GAP = 180;
const ROW_GAP = 120;

function diagramToFlow(
  diagram: DiagramModel,
  onDoubleClick?: (typeName: string) => void
): { nodes: Node<DiagramNodeData>[]; edges: Edge[] } {
  const portUsage: Record<string, Set<string>> = {};
  for (const c of diagram.connections) {
    const { nodeId: fromId, handleId: fromH } = pathToNodeAndHandle(c.from);
    const { nodeId: toId, handleId: toH } = pathToNodeAndHandle(c.to);
    portUsage[fromId] = portUsage[fromId] ?? new Set();
    portUsage[fromId].add(fromH);
    portUsage[toId] = portUsage[toId] ?? new Set();
    portUsage[toId].add(toH);
  }

  const layout = diagram.layout ?? {};
  const nodes: Node<DiagramNodeData>[] = diagram.components.map((comp, i) => {
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

    const ports = portUsage[comp.name] ? Array.from(portUsage[comp.name]) : ["p"];

    return {
      id: comp.name,
      type: "component",
      position,
      data: {
        typeName: comp.typeName,
        portHandles: ports,
        icon: comp.icon,
        rotation: comp.rotation,
        params: comp.params,
        connectorKind: comp.connectorKind,
        onDoubleClick,
      },
    };
  });

  const edges: Edge[] = diagram.connections.map((conn, i) => {
    const a = pathToNodeAndHandle(conn.from);
    const b = pathToNodeAndHandle(conn.to);
    return {
      id: `e-${conn.from}-${conn.to}-${i}`,
      source: a.nodeId,
      target: b.nodeId,
      sourceHandle: a.handleId,
      targetHandle: b.handleId,
      type: conn.line ? "polyline" : "default",
      data: conn.line ? { linePoints: conn.line.points } : undefined,
    };
  });

  return { nodes, edges };
}

function flowToDiagram(
  nodes: Node<DiagramNodeData>[],
  edges: Edge[]
): {
  components: { name: string; typeName: string }[];
  connections: { from: string; to: string }[];
  layout: Record<string, LayoutPoint>;
} {
  const components = nodes.map((n) => ({
    name: n.id,
    typeName: (n.data?.typeName as string) || "Block",
  }));
  const connections = edges.map((e) => ({
    from: nodeAndHandleToPath(e.source, e.sourceHandle ?? "p"),
    to: nodeAndHandleToPath(e.target, e.targetHandle ?? "p"),
  }));
  const layout: Record<string, LayoutPoint> = {};
  nodes.forEach((n) => {
    layout[n.id] = { x: n.position.x, y: n.position.y };
  });
  return { components, connections, layout };
}

const DEBOUNCE_MS = 600;
const FIT_VIEW_OPTIONS = { padding: 0.2 };

export interface InstantiableClass {
  name: string;
  path?: string;
}

function uniqueInstanceName(typeName: string, existingIds: string[]): string {
  const base =
    typeName.length > 0
      ? typeName[0].toLowerCase() + typeName.slice(1)
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
  onNavigateToType?: (typeName: string) => void;
}

export function DiagramView({
  source,
  projectDir,
  relativeFilePath,
  onContentChange,
  readOnly = false,
  onNavigateToType,
}: DiagramViewProps) {
  const [diagram, setDiagram] = useState<DiagramModel | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [conflictPending, setConflictPending] = useState<DiagramModel | null>(null);

  const sourceRef = useRef(source);
  sourceRef.current = source;
  const projectDirRef = useRef(projectDir);
  projectDirRef.current = projectDir;
  const filePathRef = useRef(relativeFilePath);
  filePathRef.current = relativeFilePath;
  const onContentChangeRef = useRef(onContentChange);
  onContentChangeRef.current = onContentChange;

  const handleDoubleClick = useCallback(
    (typeName: string) => {
      onNavigateToType?.(typeName);
    },
    [onNavigateToType]
  );

  const handleDoubleClickRef = useRef(handleDoubleClick);
  handleDoubleClickRef.current = handleDoubleClick;

  const initial = useMemo(() => {
    const { nodes, edges } = diagramToFlow(
      diagram ?? { modelName: "", components: [], connections: [] },
      handleDoubleClickRef.current
    );
    return { nodes, edges };
  }, [!!diagram]);

  const [nodes, setNodes, onNodesChange] = useNodesState(initial.nodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(initial.edges);

  const nodesRef = useRef(nodes);
  nodesRef.current = nodes;
  const edgesRef = useRef(edges);
  edgesRef.current = edges;

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setConflictPending(null);
    invoke<DiagramModel>("get_diagram_data_from_source", {
      source,
      projectDir: projectDir ?? undefined,
      relativePath: relativeFilePath ?? undefined,
    })
      .then((data) => {
        if (cancelled) return;
        const current = flowToDiagram(nodesRef.current, edgesRef.current);
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
          setDiagram(data);
          const { nodes: n, edges: e } = diagramToFlow(data, handleDoubleClickRef.current);
          setNodes(n);
          setEdges(e);
          setConflictPending(null);
          lastAppliedRef.current = JSON.stringify({
            components: data.components.map((c) => ({ name: c.name, typeName: c.typeName })),
            connections: data.connections.map((c) => ({ from: c.from, to: c.to })),
            layout: data.layout ?? {},
          });
        } else {
          setConflictPending(data);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(String(err));
          setDiagram(null);
          setNodes([]);
          setEdges([]);
          setConflictPending(null);
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [source, setNodes, setEdges]);

  const onConnect = useCallback(
    (conn: Connection) => {
      if (readOnly) return;
      setEdges((eds) =>
        addEdge(
          {
            ...conn,
            sourceHandle: conn.sourceHandle ?? "p",
            targetHandle: conn.targetHandle ?? "p",
          },
          eds
        )
      );
    },
    [readOnly, setEdges]
  );

  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastAppliedRef = useRef<string>("");

  const syncToSource = useCallback(() => {
    if (readOnly || !onContentChangeRef.current) return;
    const { components, connections, layout } = flowToDiagram(nodesRef.current, edgesRef.current);
    const key = JSON.stringify({ components, connections, layout });
    if (key === lastAppliedRef.current) return;
    lastAppliedRef.current = key;
    const layoutForBackend = Object.keys(layout).length > 0 ? layout : undefined;
    invoke<{ newSource: string }>("apply_diagram_edits", {
      source: sourceRef.current,
      components,
      connections,
      layout: layoutForBackend,
      projectDir: projectDirRef.current ?? undefined,
      relativePath: filePathRef.current ?? undefined,
    })
      .then(({ newSource }) => {
        onContentChangeRef.current?.(newSource);
      })
      .catch(() => {});
  }, [readOnly]);

  useEffect(() => {
    if (readOnly || !onContentChange || !diagram) return;
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(syncToSource, DEBOUNCE_MS);
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [nodes, edges, readOnly, onContentChange, diagram, syncToSource]);

  const onRefreshDiagram = useCallback(
    (data: DiagramModel) => {
      setDiagram(data);
      const { nodes: n, edges: e } = diagramToFlow(data, handleDoubleClickRef.current);
      setNodes(n);
      setEdges(e);
      setConflictPending(null);
      lastAppliedRef.current = JSON.stringify({
        components: data.components.map((c) => ({ name: c.name, typeName: c.typeName })),
        connections: data.connections.map((c) => ({ from: c.from, to: c.to })),
        layout: data.layout ?? {},
      });
    },
    [setNodes, setEdges]
  );

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
    <ReactFlowProvider>
      <DiagramFlowWithLibrary
        nodes={nodes}
        setNodes={setNodes}
        edges={edges}
        onEdgesChange={onEdgesChange}
        onNodesChange={onNodesChange}
        onConnect={onConnect}
        diagram={diagram}
        readOnly={readOnly}
        projectDir={projectDir}
        conflictPending={conflictPending}
        onRefreshDiagram={onRefreshDiagram}
      />
    </ReactFlowProvider>
  );
}

// ---------------------------------------------------------------------------
// Main flow wrapper with library sidebar
// ---------------------------------------------------------------------------

const DRAG_TYPE = "application/modelica-type";

interface DiagramFlowWithLibraryProps {
  nodes: Node<DiagramNodeData>[];
  setNodes: React.Dispatch<React.SetStateAction<Node<DiagramNodeData>[]>>;
  edges: Edge[];
  onEdgesChange: (changes: import("@xyflow/react").EdgeChange[]) => void;
  onNodesChange: (changes: import("@xyflow/react").NodeChange<Node<DiagramNodeData>>[]) => void;
  onConnect: (conn: Connection) => void;
  diagram: DiagramModel;
  readOnly: boolean;
  projectDir: string | null;
  conflictPending: DiagramModel | null;
  onRefreshDiagram: (data: DiagramModel) => void;
}

function DiagramFlowWithLibrary({
  nodes,
  setNodes,
  edges,
  onEdgesChange,
  onNodesChange,
  onConnect,
  diagram,
  readOnly,
  projectDir,
  conflictPending,
  onRefreshDiagram,
}: DiagramFlowWithLibraryProps) {
  const { screenToFlowPosition } = useReactFlow();
  const [libraryClasses, setLibraryClasses] = useState<InstantiableClass[]>([]);

  useEffect(() => {
    if (!projectDir) {
      setLibraryClasses([]);
      return;
    }
    let cancelled = false;
    invoke<InstantiableClass[]>("list_instantiable_classes", { projectDir })
      .then((list) => {
        if (!cancelled) setLibraryClasses(list);
      })
      .catch(() => {
        if (!cancelled) setLibraryClasses([]);
      });
    return () => {
      cancelled = true;
    };
  }, [projectDir]);

  const onDrop = useCallback(
    (event: React.DragEvent) => {
      if (readOnly) return;
      const typeName = event.dataTransfer.getData(DRAG_TYPE);
      if (!typeName) return;
      event.preventDefault();
      const position = screenToFlowPosition({ x: event.clientX, y: event.clientY });
      setNodes((nds) => {
        const existingIds = nds.map((n) => n.id);
        const id = uniqueInstanceName(typeName, existingIds);
        return nds.concat({
          id,
          type: "component",
          position,
          data: { typeName, portHandles: ["p"] },
        });
      });
    },
    [readOnly, setNodes, screenToFlowPosition]
  );
  const onDragOver = useCallback((event: React.DragEvent) => {
    if (event.dataTransfer.types.includes(DRAG_TYPE)) event.preventDefault();
  }, []);

  return (
    <div className="h-full w-full flex flex-col">
      {conflictPending && (
        <div
          className="shrink-0 flex items-center justify-between gap-2 px-3 py-2 bg-amber-500/20 border-b border-amber-500/40 text-[var(--text)]"
          role="alert"
        >
          <span className="text-sm">{t("diagramConflict")}</span>
          <button
            type="button"
            className="px-2 py-1 text-xs font-medium rounded bg-primary text-white hover:opacity-90"
            onClick={() => onRefreshDiagram(conflictPending)}
          >
            {t("refreshDiagram")}
          </button>
        </div>
      )}
      <div className="flex-1 min-h-0 flex">
      {projectDir && libraryClasses.length > 0 && !readOnly && (
        <div
          className="w-48 shrink-0 border-r border-[var(--border)] flex flex-col bg-[var(--bg-elevated)] overflow-auto"
          aria-label={t("componentLibrary")}
        >
          <div className="p-2 text-xs font-medium text-[var(--text-muted)] border-b border-[var(--border)]">
            {t("componentLibrary")}
          </div>
          <ul className="p-1 list-none">
            {libraryClasses.map((c) => (
              <li
                key={c.name}
                draggable
                onDragStart={(e) => {
                  e.dataTransfer.setData(DRAG_TYPE, c.name);
                  e.dataTransfer.effectAllowed = "copy";
                }}
                className="px-2 py-1.5 text-xs text-[var(--text)] cursor-grab active:cursor-grabbing rounded hover:bg-white/10"
              >
                {c.name}
              </li>
            ))}
          </ul>
        </div>
      )}
      <div className="flex-1 min-w-0" onDrop={onDrop} onDragOver={onDragOver}>
        <ReactFlow
          nodes={nodes}
          edges={edges}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          fitView
          fitViewOptions={FIT_VIEW_OPTIONS}
          nodesDraggable={!readOnly}
          nodesConnectable={!readOnly}
          elementsSelectable={!readOnly}
          deleteKeyCode={readOnly ? null : "Delete"}
        >
          <Background />
          <Controls />
          <MiniMap />
          {diagram.modelName && (
            <Panel position="top-left" className="text-xs text-[var(--text-muted)]">
              {diagram.modelName}
            </Panel>
          )}
        </ReactFlow>
      </div>
      </div>
    </div>
  );
}
