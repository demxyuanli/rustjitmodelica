import {
  Background,
  Controls,
  MiniMap,
  ReactFlow,
  ReactFlowProvider,
  type Connection,
  type Edge,
  type EdgeProps,
  type Node,
  type NodeProps,
  BaseEdge,
  Handle,
  Position,
} from "@xyflow/react";
import React, { useMemo } from "react";
import { connectorHandleStyle, DEFAULT_ICON_SIZE, IconSvg, type AnnotationPoint, type IconDiagramAnnotation } from "./DiagramSvgRenderer";

export interface DiagramNodeData {
  [key: string]: unknown;
  typeName: string;
  libraryId?: string;
  portHandles: string[];
  icon?: IconDiagramAnnotation;
  rotation?: number;
  params?: { name: string; value: string }[];
  connectorKind?: string;
  isInput?: boolean;
  isOutput?: boolean;
  onDoubleClick?: (typeName: string, libraryId?: string) => void;
}

const DEFAULT_HANDLE_ID = "__default__";

const FIT_VIEW_OPTIONS = { padding: 0.2 };

const ComponentNode = React.memo(function ComponentNode(props: NodeProps<Node<DiagramNodeData>>) {
  const { id, data, selected } = props;
  const safeData = data ?? { typeName: "Block", portHandles: [DEFAULT_HANDLE_ID] };
  const ports = safeData.portHandles?.length ? safeData.portHandles : [DEFAULT_HANDLE_ID];
  const hasIcon = safeData.icon && safeData.icon.graphics && safeData.icon.graphics.length > 0;
  const iconSize = DEFAULT_ICON_SIZE;

  const paramStr = safeData.params
    ?.filter((item) => item.value)
    .map((item) => (item.name ? `${item.name}=${item.value}` : item.value))
    .join(", ");

  const portStyles = useMemo(
    () =>
      ports.map((_port, index) => {
        const pct = ports.length === 1 ? 50 : 20 + (index * 60) / Math.max(1, ports.length - 1);
        return {
          right: {
            ...connectorHandleStyle(safeData.connectorKind, "right"),
            top: `${pct}%`,
            transform: "translateY(-50%)",
          } as React.CSSProperties,
          left: {
            ...connectorHandleStyle(safeData.connectorKind, "left"),
            top: `${pct}%`,
            transform: "translateY(-50%)",
          } as React.CSSProperties,
        };
      }),
    [ports, safeData.connectorKind],
  );

  return (
    <div
      className={`rounded border bg-[var(--bg-elevated)] border-[var(--border)] relative ${selected ? "ring-2 ring-primary" : ""}`}
      style={{ minWidth: hasIcon ? iconSize + 16 : 80, padding: hasIcon ? 4 : "8px 12px" }}
      onDoubleClick={() => safeData.onDoubleClick?.(safeData.typeName, safeData.libraryId as string | undefined)}
    >
      {hasIcon ? (
        <div className="flex flex-col items-center gap-0.5">
          <IconSvg icon={safeData.icon!} instanceName={id} rotation={safeData.rotation} size={iconSize} />
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
      {ports.map((port, index) => (
        <React.Fragment key={port}>
          <Handle type="source" id={port} position={Position.Right} style={portStyles[index].right} />
          <Handle type="target" id={port} position={Position.Left} style={portStyles[index].left} />
        </React.Fragment>
      ))}
    </div>
  );
});

const PolylineEdge = React.memo(function PolylineEdge(props: EdgeProps) {
  const { sourceX, sourceY, targetX, targetY, style, markerEnd } = props;
  const edgeData = (props as { data?: { linePoints?: AnnotationPoint[] } }).data;
  if (edgeData?.linePoints && edgeData.linePoints.length >= 2) {
    const d = edgeData.linePoints
      .map((point, index) => `${index === 0 ? "M" : "L"} ${point.x} ${point.y}`)
      .join(" ");
    return <BaseEdge path={d} style={style} markerEnd={markerEnd} />;
  }
  const midX = (sourceX + targetX) / 2;
  return (
    <BaseEdge
      path={`M ${sourceX} ${sourceY} C ${midX} ${sourceY}, ${midX} ${targetY}, ${targetX} ${targetY}`}
      style={style}
      markerEnd={markerEnd}
    />
  );
});

const nodeTypes = { component: ComponentNode as React.ComponentType<NodeProps<Node<DiagramNodeData>>> };
const edgeTypes = { polyline: PolylineEdge };

export interface DiagramEditorViewProps {
  nodes: Node<DiagramNodeData>[];
  edges: Edge[];
  readOnly: boolean;
  onNodesChange: (changes: any) => void;
  onEdgesChange: (changes: any) => void;
  onConnect: (connection: Connection) => void;
}

export function DiagramEditorView({
  nodes,
  edges,
  readOnly,
  onNodesChange,
  onEdgesChange,
  onConnect,
}: DiagramEditorViewProps) {
  return (
    <ReactFlowProvider>
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
        elementsSelectable={true}
        deleteKeyCode={readOnly ? null : "Delete"}
      >
        <Background />
        <Controls />
        <MiniMap />
      </ReactFlow>
    </ReactFlowProvider>
  );
}
