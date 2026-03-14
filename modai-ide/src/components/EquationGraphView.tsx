import { useEffect, useMemo, useState } from "react";
import {
  ReactFlow,
  ReactFlowProvider,
  useNodesState,
  useEdgesState,
  type Node,
  type Edge,
  type NodeProps,
  Handle,
  Position,
  Background,
  Controls,
  Panel,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { getEquationGraph } from "../api/tauri";
import type { EquationGraph } from "../types";
import { t } from "../i18n";

const LAYER_GAP = 220;
const NODE_GAP = 48;

function equationGraphToFlow(
  g: EquationGraph
): { nodes: Node[]; edges: Edge[] } {
  const eqNodes = g.nodes.filter((n) => n.kind === "equation");
  const varNodes = g.nodes.filter((n) => n.kind === "variable");
  const nodes: Node[] = [];
  let y = 0;
  for (const n of eqNodes) {
    nodes.push({
      id: n.id,
      type: "equation",
      data: { label: n.label },
      position: { x: 0, y },
    });
    y += NODE_GAP;
  }
  y = 0;
  for (const n of varNodes) {
    nodes.push({
      id: n.id,
      type: "variable",
      data: { label: n.label },
      position: { x: LAYER_GAP, y },
    });
    y += NODE_GAP;
  }
  const edges: Edge[] = g.edges.map((e, i) => ({
    id: `e${i}`,
    source: e.source,
    target: e.target,
    type: e.kind === "solves" ? "smoothstep" : "default",
    data: e.kind === "solves" ? { label: "solves" } : undefined,
  }));
  return { nodes, edges };
}

function EquationNode({ data }: NodeProps) {
  const label = (data?.label as string) ?? "";
  return (
    <div
      className="px-2 py-1 rounded border border-amber-600/80 bg-amber-950/50 text-amber-200 text-xs font-mono max-w-[180px] truncate"
      title={label}
    >
      <Handle type="target" position={Position.Left} className="!w-1.5 !h-1.5 !-left-1" />
      {label}
      <Handle type="source" position={Position.Right} className="!w-1.5 !h-1.5 !-right-1" />
    </div>
  );
}

function VariableNode({ data }: NodeProps) {
  const label = (data?.label as string) ?? "";
  return (
    <div
      className="px-2 py-1 rounded border border-sky-600/80 bg-sky-950/50 text-sky-200 text-xs font-mono max-w-[180px] truncate"
      title={label}
    >
      <Handle type="target" position={Position.Left} className="!w-1.5 !h-1.5 !-left-1" />
      {label}
      <Handle type="source" position={Position.Right} className="!w-1.5 !h-1.5 !-right-1" />
    </div>
  );
}

const nodeTypes = { equation: EquationNode, variable: VariableNode };

interface EquationGraphViewProps {
  code: string;
  modelName: string;
  projectDir: string | null | undefined;
}

export function EquationGraphView({ code, modelName, projectDir }: EquationGraphViewProps) {
  const [graph, setGraph] = useState<EquationGraph | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    getEquationGraph(code, modelName, projectDir)
      .then((g) => {
        if (!cancelled) setGraph(g);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [code, modelName, projectDir]);

  const { nodes: initialNodes, edges: initialEdges } = useMemo(() => {
    if (!graph) return { nodes: [], edges: [] };
    return equationGraphToFlow(graph);
  }, [graph]);

  const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges);

  useEffect(() => {
    setNodes(initialNodes);
    setEdges(initialEdges);
  }, [initialNodes, initialEdges, setNodes, setEdges]);

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center text-[var(--text-muted)] text-sm">
        {t("loading")}...
      </div>
    );
  }
  if (error) {
    return (
      <div className="h-full flex items-center justify-center p-4">
        <div className="text-red-400 text-sm max-w-md">{error}</div>
      </div>
    );
  }
  if (!graph || graph.nodes.length === 0) {
    return (
      <div className="h-full flex items-center justify-center text-[var(--text-muted)] text-sm">
        {t("equationGraphEmpty")}
      </div>
    );
  }

  return (
    <ReactFlowProvider>
      <div className="h-full w-full">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          nodeTypes={nodeTypes}
          fitView
          fitViewOptions={{ padding: 0.2 }}
          minZoom={0.1}
          maxZoom={2}
        >
          <Background />
          <Controls />
          <Panel position="top-left" className="text-xs text-[var(--text-muted)]">
            {graph.nodes.filter((n) => n.kind === "equation").length} eq / {graph.nodes.filter((n) => n.kind === "variable").length} var
          </Panel>
        </ReactFlow>
      </div>
    </ReactFlowProvider>
  );
}
