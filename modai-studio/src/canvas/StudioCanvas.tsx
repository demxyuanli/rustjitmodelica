import { useCallback, useEffect, useMemo } from "react";
import {
  Background,
  ConnectionMode,
  Controls,
  MiniMap,
  ReactFlow,
  addEdge,
  useEdgesState,
  useNodesState,
  type Connection,
  type Edge,
  type OnSelectionChangeParams,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import type { CatalogEntry, DiagramDocument } from "../api/types";
import { StudioBlockNode, type StudioFlowNode } from "./StudioBlockNode";
import {
  canConnect,
  createNodeFromCatalog,
  findNode,
  portOf,
  upsertEdge,
} from "./connectionRules";
import { snapToGrid, validateEdges } from "./editOps";

const nodeTypes = { studio: StudioBlockNode };

function toFlow(diagram: DiagramDocument): { nodes: StudioFlowNode[]; edges: Edge[] } {
  const validity = new Map(validateEdges(diagram).map((v) => [v.id, v]));
  return {
    nodes: diagram.nodes.map((n) => ({
      id: n.id,
      type: "studio" as const,
      position: n.position,
      data: { diagram: n },
      selected: false,
    })),
    edges: diagram.edges.map((e) => {
      const v = validity.get(e.id);
      const signal = findNode(diagram, e.sourceNodeId)?.domain === "signal";
      const bad = v && !v.ok;
      return {
        id: e.id,
        source: e.sourceNodeId,
        target: e.targetNodeId,
        sourceHandle: e.sourcePortId,
        targetHandle: e.targetPortId,
        label: e.signalName ?? undefined,
        style: {
          stroke: bad ? "#e35d6a" : signal ? "#5b9bd5" : "#c47a3a",
          strokeWidth: bad ? 2.5 : 1.5,
        },
        animated: !!bad,
      };
    }),
  };
}

export interface StudioCanvasProps {
  diagram: DiagramDocument;
  diagramRev: number;
  selectedIds: string[];
  snapEnabled: boolean;
  onDiagramChange: (d: DiagramDocument) => void;
  onSelectionChange: (nodeIds: string[]) => void;
  pendingCatalog: CatalogEntry | null;
  onConsumePendingCatalog: () => void;
  focusNodeId?: string | null;
  onNodeDoubleClick?: (nodeId: string) => void;
}

export function StudioCanvas({
  diagram,
  diagramRev,
  selectedIds,
  snapEnabled,
  onDiagramChange,
  onSelectionChange,
  pendingCatalog,
  onConsumePendingCatalog,
  focusNodeId,
  onNodeDoubleClick,
}: StudioCanvasProps) {
  const [nodes, setNodes, onNodesChange] = useNodesState<StudioFlowNode>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);

  useEffect(() => {
    const f = toFlow(diagram);
    setNodes(
      f.nodes.map((n) => ({
        ...n,
        selected: selectedIds.includes(n.id),
      })),
    );
    setEdges(f.edges);
  }, [diagramRev, setNodes, setEdges]);

  useEffect(() => {
    setNodes((nds) =>
      nds.map((n) => ({
        ...n,
        selected: selectedIds.includes(n.id),
      })),
    );
  }, [selectedIds, setNodes]);

  useEffect(() => {
    if (!focusNodeId) return;
    setNodes((nds) =>
      nds.map((n) => ({
        ...n,
        selected: n.id === focusNodeId,
      })),
    );
  }, [focusNodeId, setNodes]);

  const pushDiagram = useCallback(
    (nextNodes: StudioFlowNode[], nextEdges: Edge[]) => {
      const byId = new Map(diagram.nodes.map((n) => [n.id, n]));
      const nodesOut = nextNodes.map((fn) => {
        const prev = byId.get(fn.id) ?? fn.data.diagram;
        const pos = snapEnabled
          ? snapToGrid(fn.position.x, fn.position.y)
          : { x: fn.position.x, y: fn.position.y };
        return {
          ...prev,
          ...fn.data.diagram,
          id: fn.id,
          position: pos,
        };
      });
      onDiagramChange({
        ...diagram,
        nodes: nodesOut,
        edges: nextEdges.map((e) => ({
          id: e.id,
          sourceNodeId: e.source,
          targetNodeId: e.target,
          sourcePortId: String(e.sourceHandle ?? ""),
          targetPortId: String(e.targetHandle ?? ""),
          signalName: typeof e.label === "string" ? e.label : null,
          vertices: [],
          routing: "orthogonal",
        })),
      });
    },
    [diagram, onDiagramChange, snapEnabled],
  );

  const onConnect = useCallback(
    (c: Connection) => {
      if (!c.source || !c.target || !c.sourceHandle || !c.targetHandle) return;
      if (c.source === c.target) return;
      const sn = findNode(diagram, c.source);
      const tn = findNode(diagram, c.target);
      if (!sn || !tn) return;
      const decision = canConnect(
        portOf(sn, c.sourceHandle),
        portOf(tn, c.targetHandle),
        sn.domain,
        tn.domain,
      );
      if (!decision.ok) {
        window.alert(decision.reason ?? "invalid connection");
        return;
      }
      const edge = {
        id: `e_${Date.now()}`,
        sourceNodeId: c.source,
        sourcePortId: c.sourceHandle,
        targetNodeId: c.target,
        targetPortId: c.targetHandle,
        signalName: `${sn.instanceName}.${c.sourceHandle}`,
        vertices: [],
        routing: "orthogonal" as const,
      };
      onDiagramChange(upsertEdge(diagram, edge));
      setEdges((eds) =>
        addEdge(
          {
            ...c,
            id: edge.id,
            label: edge.signalName ?? undefined,
            style: { stroke: sn.domain === "signal" ? "#5b9bd5" : "#c47a3a" },
          },
          eds,
        ),
      );
    },
    [diagram, onDiagramChange, setEdges],
  );

  const onPaneClick = useCallback(
    (ev: React.MouseEvent) => {
      if (!pendingCatalog) return;
      const bounds = (ev.currentTarget as HTMLElement).getBoundingClientRect();
      let pos = {
        x: ev.clientX - bounds.left - 40,
        y: ev.clientY - bounds.top - 20,
      };
      if (snapEnabled) pos = snapToGrid(pos.x, pos.y);
      const node = createNodeFromCatalog(pendingCatalog, pos, diagram.nodes);
      const flowNode: StudioFlowNode = {
        id: node.id,
        type: "studio",
        position: node.position,
        data: { diagram: node },
      };
      setNodes((nds) => [...nds, flowNode]);
      onDiagramChange({ ...diagram, nodes: [...diagram.nodes, node] });
      onConsumePendingCatalog();
    },
    [pendingCatalog, diagram, onDiagramChange, onConsumePendingCatalog, setNodes, snapEnabled],
  );

  const onSel = useCallback(
    (p: OnSelectionChangeParams) => {
      onSelectionChange(p.nodes.map((n) => n.id));
    },
    [onSelectionChange],
  );

  const invalidCount = useMemo(
    () => validateEdges(diagram).filter((v) => !v.ok).length,
    [diagram],
  );

  return (
    <div className="canvas-wrap" style={{ width: "100%", height: "100%" }}>
      {invalidCount > 0 ? (
        <div className="canvas-banner">
          {invalidCount} invalid connection(s) highlighted in red
        </div>
      ) : null}
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={(chs) => {
          onEdgesChange(chs);
          const removed = chs
            .filter((c) => c.type === "remove")
            .map((c) => ("id" in c ? String(c.id) : ""));
          if (removed.length) {
            onDiagramChange({
              ...diagram,
              edges: diagram.edges.filter((e) => !removed.includes(e.id)),
            });
          }
        }}
        onConnect={onConnect}
        onPaneClick={onPaneClick}
        onSelectionChange={onSel}
        onNodeDragStop={(_e, _n, ns) => pushDiagram(ns as StudioFlowNode[], edges)}
        onNodeDoubleClick={(_e, node) => onNodeDoubleClick?.(node.id)}
        nodeTypes={nodeTypes}
        connectionMode={ConnectionMode.Loose}
        fitView
        snapToGrid={snapEnabled}
        snapGrid={[16, 16]}
        deleteKeyCode={["Backspace", "Delete"]}
        onNodesDelete={(deleted) => {
          const ids = new Set(deleted.map((n) => n.id));
          onDiagramChange({
            ...diagram,
            nodes: diagram.nodes.filter((n) => !ids.has(n.id)),
            edges: diagram.edges.filter((e) => !ids.has(e.sourceNodeId) && !ids.has(e.targetNodeId)),
          });
        }}
      >
        <Background gap={16} color="#2a3038" />
        <Controls />
        <MiniMap />
      </ReactFlow>
    </div>
  );
}
