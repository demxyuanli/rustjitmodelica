import { useEffect, useState } from "react";
import ELK from "elkjs/lib/elk.bundled.js";
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
  MarkerType,
  type ReactFlowInstance,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { getEquationGraph } from "../api/tauri";
import type { EquationGraph } from "../types";
import { t } from "../i18n";

const elk = new ELK();
const DEFAULT_NODE_HEIGHT = 42;
const MIN_NODE_WIDTH = 180;
const MAX_NODE_WIDTH = 320;
const LABEL_CHAR_WIDTH = 7.2;

type GraphNodeData = {
  label: string;
  width: number;
  height: number;
};

function estimateNodeWidth(label: string): number {
  return Math.max(MIN_NODE_WIDTH, Math.min(MAX_NODE_WIDTH, Math.ceil(label.length * LABEL_CHAR_WIDTH) + 32));
}

function buildBaseFlow(g: EquationGraph): { nodes: Node[]; edges: Edge[] } {
  const nodes: Node[] = [];
  for (const n of g.nodes) {
    const width = estimateNodeWidth(n.label);
    nodes.push({
      id: n.id,
      type: n.kind === "equation" ? "equation" : "variable",
      data: {
        label: n.label,
        width,
        height: DEFAULT_NODE_HEIGHT,
      },
      position: { x: 0, y: 0 },
      sourcePosition: Position.Right,
      targetPosition: Position.Left,
    });
  }
  const edges: Edge[] = g.edges.map((e, i) => ({
    id: `e${i}`,
    source: e.source,
    target: e.target,
    type: "smoothstep",
    animated: e.kind === "solves",
    label: e.kind === "solves" ? "solves" : undefined,
    markerEnd: {
      type: MarkerType.ArrowClosed,
      width: 16,
      height: 16,
    },
    style: {
      stroke: e.kind === "solves" ? "#f59e0b" : "#60a5fa",
      strokeWidth: e.kind === "solves" ? 1.8 : 1.4,
    },
  }));
  return { nodes, edges };
}

async function layoutEquationGraph(g: EquationGraph): Promise<{ nodes: Node[]; edges: Edge[] }> {
  const base = buildBaseFlow(g);
  const layoutGraph = await elk.layout({
    id: "equation-graph",
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": "RIGHT",
      "elk.padding": "[top=24,left=24,bottom=24,right=24]",
      "elk.layered.spacing.nodeNodeBetweenLayers": "120",
      "elk.spacing.nodeNode": "56",
      "elk.edgeRouting": "ORTHOGONAL",
      "elk.layered.nodePlacement.strategy": "NETWORK_SIMPLEX",
      "elk.layered.crossingMinimization.strategy": "LAYER_SWEEP",
    },
    children: base.nodes.map((node) => {
      const nodeData = node.data as GraphNodeData;
      return {
        id: node.id,
        width: nodeData.width,
        height: nodeData.height,
      };
    }),
    edges: g.edges.map((edge, index) => ({
      id: `e${index}`,
      sources: [edge.source],
      targets: [edge.target],
    })),
  });

  const positions = new Map(
    (layoutGraph.children ?? []).map((child) => [child.id, { x: child.x ?? 0, y: child.y ?? 0 }])
  );

  return {
    nodes: base.nodes.map((node) => ({
      ...node,
      position: positions.get(node.id) ?? { x: 0, y: 0 },
    })),
    edges: base.edges,
  };
}

function EquationNode({ data }: NodeProps) {
  const nodeData = data as GraphNodeData | undefined;
  const label = nodeData?.label ?? "";
  return (
    <div
      className="rounded border border-amber-500/70 bg-amber-950/40 px-3 py-2 font-mono text-xs text-amber-200 shadow-[0_10px_20px_rgba(0,0,0,0.18)]"
      title={label}
      style={{ width: nodeData?.width }}
    >
      <Handle type="target" position={Position.Left} className="!w-1.5 !h-1.5 !-left-1" />
      <div className="truncate">{label}</div>
      <Handle type="source" position={Position.Right} className="!w-1.5 !h-1.5 !-right-1" />
    </div>
  );
}

function VariableNode({ data }: NodeProps) {
  const nodeData = data as GraphNodeData | undefined;
  const label = nodeData?.label ?? "";
  return (
    <div
      className="rounded border border-sky-500/70 bg-sky-950/35 px-3 py-2 font-mono text-xs text-sky-200 shadow-[0_10px_20px_rgba(0,0,0,0.18)]"
      title={label}
      style={{ width: nodeData?.width }}
    >
      <Handle type="target" position={Position.Left} className="!w-1.5 !h-1.5 !-left-1" />
      <div className="truncate">{label}</div>
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
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const [flowInstance, setFlowInstance] = useState<ReactFlowInstance | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function loadGraph() {
      setLoading(true);
      setError(null);
      try {
        const graphResult = await getEquationGraph(code, modelName, projectDir);
        if (cancelled) return;
        setGraph(graphResult);
        if (graphResult.nodes.length === 0) {
          setNodes([]);
          setEdges([]);
          return;
        }
        const layout = await layoutEquationGraph(graphResult);
        if (cancelled) return;
        setNodes(layout.nodes);
        setEdges(layout.edges);
      } catch (layoutError) {
        if (!cancelled) {
          setError(String(layoutError));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void loadGraph();

    return () => {
      cancelled = true;
    };
  }, [code, modelName, projectDir, setEdges, setNodes]);

  useEffect(() => {
    if (!flowInstance || nodes.length === 0) return;
    const rafId = window.requestAnimationFrame(() => {
      void flowInstance.fitView({ padding: 0.16, duration: 260 });
    });
    return () => window.cancelAnimationFrame(rafId);
  }, [flowInstance, nodes, edges]);

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
          onInit={setFlowInstance}
          fitView
          fitViewOptions={{ padding: 0.16 }}
          minZoom={0.05}
          maxZoom={2.5}
          nodesDraggable={false}
          nodesConnectable={false}
          elementsSelectable
          proOptions={{ hideAttribution: true }}
        >
          <Background />
          <Controls />
          <Panel position="top-left" className="text-xs text-[var(--text-muted)]">
            {graph.nodes.filter((n) => n.kind === "equation").length} eq / {graph.nodes.filter((n) => n.kind === "variable").length} var / {t("horizontalAutoLayout")}
          </Panel>
        </ReactFlow>
      </div>
    </ReactFlowProvider>
  );
}
