import { useEffect, useRef, useState } from "react";
import ELK from "elkjs/lib/elk.bundled.js";
import { dia, shapes } from "@joint/core";
import { createPaper, createPaperHandle, resolveDiagramColors, type JointPaperHandle } from "../utils/jointUtils";
import { useDiagramScheme } from "../contexts/DiagramSchemeContext";
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
  kind: string;
};

function estimateNodeWidth(label: string): number {
  return Math.max(MIN_NODE_WIDTH, Math.min(MAX_NODE_WIDTH, Math.ceil(label.length * LABEL_CHAR_WIDTH) + 32));
}

function colorToRgba(cssColor: string, alpha: number): string {
  const hex = cssColor.match(/^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i);
  if (hex) {
    return `rgba(${parseInt(hex[1], 16)}, ${parseInt(hex[2], 16)}, ${parseInt(hex[3], 16)}, ${alpha})`;
  }
  const rgb = cssColor.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
  if (rgb) {
    return `rgba(${rgb[1]}, ${rgb[2]}, ${rgb[3]}, ${alpha})`;
  }
  return cssColor;
}

export type LayoutAlgorithm = "layered" | "box" | "force";
export type LayoutDirection = "RIGHT" | "DOWN" | "LEFT" | "UP";

export interface EquationGraphLayoutOptions {
  algorithm?: LayoutAlgorithm;
  direction?: LayoutDirection;
}

const DEFAULT_LAYOUT: EquationGraphLayoutOptions = {
  algorithm: "layered",
  direction: "RIGHT",
};

function buildElkOptions(options: EquationGraphLayoutOptions): Record<string, string> {
  const algorithm = options.algorithm ?? "layered";
  const direction = options.direction ?? "RIGHT";
  const base: Record<string, string> = {
    "elk.padding": "[top=24,left=24,bottom=24,right=24]",
    "elk.spacing.nodeNode": "56",
  };
  if (algorithm === "layered") {
    base["elk.algorithm"] = "layered";
    base["elk.direction"] = direction;
    base["elk.layered.spacing.nodeNodeBetweenLayers"] = "120";
    base["elk.edgeRouting"] = "ORTHOGONAL";
    base["elk.layered.nodePlacement.strategy"] = "NETWORK_SIMPLEX";
    base["elk.layered.crossingMinimization.strategy"] = "LAYER_SWEEP";
  } else if (algorithm === "box") {
    base["elk.algorithm"] = "box";
    base["elk.direction"] = direction;
    base["elk.box.spacing.nodeNode"] = "40";
  } else {
    base["elk.algorithm"] = "force";
    base["elk.direction"] = direction;
  }
  return base;
}

interface LayoutResult {
  nodes: Array<{ id: string; x: number; y: number; data: GraphNodeData }>;
  edges: Array<{ id: string; source: string; target: string; kind: string }>;
}

async function layoutEquationGraph(
  g: EquationGraph,
  options: EquationGraphLayoutOptions = {}
): Promise<LayoutResult> {
  const nodeDataMap: Record<string, GraphNodeData> = {};
  for (const n of g.nodes) {
    const width = estimateNodeWidth(n.label);
    nodeDataMap[n.id] = { label: n.label, width, height: DEFAULT_NODE_HEIGHT, kind: n.kind };
  }

  const layoutOptions = buildElkOptions(options);
  const layoutGraph = await elk.layout({
    id: "equation-graph",
    layoutOptions,
    children: g.nodes.map((n) => ({
      id: n.id,
      width: nodeDataMap[n.id].width,
      height: nodeDataMap[n.id].height,
    })),
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
    nodes: g.nodes.map((n) => ({
      id: n.id,
      x: positions.get(n.id)?.x ?? 0,
      y: positions.get(n.id)?.y ?? 0,
      data: nodeDataMap[n.id],
    })),
    edges: g.edges.map((e, i) => ({
      id: `e${i}`,
      source: e.source,
      target: e.target,
      kind: e.kind,
    })),
  };
}

interface EquationGraphViewProps {
  code: string;
  modelName: string;
  projectDir: string | null | undefined;
  layoutOptions?: EquationGraphLayoutOptions;
  onReady?: (handle: JointPaperHandle | null) => void;
}

export function EquationGraphView({ code, modelName, projectDir, layoutOptions: externalLayout, onReady }: EquationGraphViewProps) {
  const [graph, setGraph] = useState<EquationGraph | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const containerRef = useRef<HTMLDivElement>(null);
  const graphRef = useRef<dia.Graph | null>(null);
  const paperRef = useRef<dia.Paper | null>(null);
  const initializedRef = useRef(false);
  const layoutOptions = externalLayout ?? DEFAULT_LAYOUT;
  const { schemeId } = useDiagramScheme();

  useEffect(() => {
    let cancelled = false;

    async function loadGraph() {
      setLoading(true);
      setError(null);
      try {
        const graphResult = await getEquationGraph(code, modelName, projectDir);
        if (cancelled) return;
        setGraph(graphResult);
      } catch (loadError) {
        if (!cancelled) {
          setError(String(loadError));
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
  }, [code, modelName, projectDir]);

  useEffect(() => {
    if (!graph || graph.nodes.length === 0 || !containerRef.current) return;
    let cancelled = false;

    void (async () => {
      const result = await layoutEquationGraph(graph, layoutOptions);
      if (cancelled) return;
      renderGraph(result);
    })();

    return () => {
      cancelled = true;
    };
  }, [graph, layoutOptions.algorithm, layoutOptions.direction, schemeId]);

  function renderGraph(result: LayoutResult) {
    const container = containerRef.current;
    if (!container) return;

    if (paperRef.current) {
      paperRef.current.remove();
      paperRef.current = null;
      graphRef.current = null;
      initializedRef.current = false;
    }

    const paperEl = document.createElement("div");
    paperEl.style.width = "100%";
    paperEl.style.height = "100%";
    container.appendChild(paperEl);

    const jointGraph = new dia.Graph({}, { cellNamespace: shapes });
    const paper = createPaper({
      el: paperEl,
      graph: jointGraph,
      gridSize: 1,
      readOnly: true,
    });

    graphRef.current = jointGraph;
    paperRef.current = paper;
    initializedRef.current = true;

    const theme = resolveDiagramColors();
    const equationFill = colorToRgba(theme.primary, 0.35);
    const variableFill = colorToRgba(theme.border, 0.2);

    for (const node of result.nodes) {
      const isEquation = node.data.kind === "equation";
      const el = new shapes.standard.Rectangle({
        id: node.id,
        position: { x: node.x, y: node.y },
        size: { width: node.data.width, height: node.data.height },
        attrs: {
          body: {
            rx: 4,
            ry: 4,
            fill: isEquation ? equationFill : variableFill,
            stroke: isEquation ? theme.primary : theme.border,
            strokeWidth: isEquation ? 2 : 1.5,
          },
          label: {
            text: node.data.label,
            fontSize: 11,
            fontFamily: "monospace",
            fill: theme.text,
            textVerticalAnchor: "middle",
            textAnchor: "middle",
          },
        },
        ports: {
          groups: {
            input: {
              position: { name: "left" },
              attrs: { portBody: { magnet: false, width: 3, height: 3, x: -1.5, y: -1.5, fill: "transparent" } },
              markup: [{ tagName: "rect", selector: "portBody" }],
            },
            output: {
              position: { name: "right" },
              attrs: { portBody: { magnet: false, width: 3, height: 3, x: -1.5, y: -1.5, fill: "transparent" } },
              markup: [{ tagName: "rect", selector: "portBody" }],
            },
          },
        },
      });
      el.addPort({ id: `${node.id}_in`, group: "input" });
      el.addPort({ id: `${node.id}_out`, group: "output" });
      jointGraph.addCell(el);
    }

    for (const edge of result.edges) {
      const isSolves = edge.kind === "solves";
      const strokeColor = isSolves ? theme.primary : theme.border;
      const link = new shapes.standard.Link({
        id: edge.id,
        source: { id: edge.source, port: `${edge.source}_out` },
        target: { id: edge.target, port: `${edge.target}_in` },
        router: { name: "manhattan", args: { step: 10 } },
        connector: { name: "rounded", args: { radius: 4 } },
        attrs: {
          line: {
            stroke: strokeColor,
            strokeWidth: isSolves ? 2.5 : 1.2,
            strokeDasharray: isSolves ? "8 4" : undefined,
            ...(isSolves ? { class: "joint-dep-link-line-animated" } : {}),
            targetMarker: {
              type: "path",
              d: "M 10 -5 0 0 10 5 Z",
              fill: strokeColor,
            },
          },
        },
        labels: isSolves
          ? [
              {
                position: 0.5,
                attrs: {
                  text: { text: "solves", fontSize: 9, fontWeight: 600, fill: theme.primary },
                  rect: { fill: "transparent" },
                },
              },
            ]
          : [],
      });
      jointGraph.addCell(link);
    }

    if (jointGraph.getElements().length > 0) {
      try {
        const containerRect = container.getBoundingClientRect();
        const isSmall = containerRect.height < 300 || containerRect.width < 400;
        paper.transformToFitContent({
          padding: isSmall ? 10 : 30,
          maxScale: isSmall ? 1.0 : 2.5,
          minScale: 0.02,
        });
      } catch (_) {
        // empty graph
      }
    }

    const handle = createPaperHandle(paper);
    onReady?.(handle);
  }

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
    <div className="h-full w-full relative">
      <div ref={containerRef} className="absolute inset-0" />
    </div>
  );
}
