import { useEffect, useRef, useState } from "react";
import ELK from "elkjs/lib/elk.bundled.js";
import { dia, shapes } from "@joint/core";
import { createPaper, createPaperHandle, resolveDiagramColors, type JointPaperHandle } from "../utils/jointUtils";
import { useDiagramScheme } from "../contexts/DiagramSchemeContext";
import {
  getEquationGraphV2,
  getMonitorEvents,
  type EquationGraphMode,
  type EquationGraphNodeKey,
} from "../api/tauri";
import type { EquationGraph } from "../types";
import { t, tf } from "../i18n";
import type { DependencyGraphBehavior } from "../utils/dependencyGraphBehavior";

const DEFAULT_DEPENDENCY_GRAPH_BEHAVIOR: DependencyGraphBehavior = {
  fullTimeoutSec: 8,
  autoDowngradeFromFull: true,
  downgradeTarget: "compact",
  initialGraphMode: "compact",
};

const elk = new ELK();
const DEFAULT_NODE_HEIGHT = 42;
const MIN_NODE_WIDTH = 180;
const MAX_NODE_WIDTH = 320;
const LABEL_CHAR_WIDTH = 7.2;

type EquationSignature = {
  index: number;
  hash: number;
};

function fnv1a32(input: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < input.length; i += 1) {
    h ^= input.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h >>> 0;
}

function normalizeEquationTokens(block: string): string {
  // Lightweight tokenization: drop comments/whitespace noise, keep operators/identifiers/numbers.
  let s = block.replace(/\/\/.*$/gm, "");
  s = s.replace(/\s+/g, " ");
  const tokens = s.match(/[A-Za-z_][A-Za-z0-9_\.]*|\d+(?:\.\d+)?(?:[eE][+\-]?\d+)?|[:=<>!+\-*/^(),\[\]{};]|./g) ?? [];
  return tokens
    .map((t) => t.trim().toLowerCase())
    .filter((t) => t.length > 0 && t !== " ")
    .join(" ");
}

function splitEquationStatements(lines: string[]): string[] {
  const out: string[] = [];
  let current = "";
  let depth = 0;
  const flush = () => {
    const v = current.trim();
    if (v) out.push(v);
    current = "";
  };
  for (const line of lines) {
    const src = line.trim();
    if (!src) continue;
    current += (current ? " " : "") + src;
    for (let i = 0; i < src.length; i += 1) {
      const ch = src[i];
      if (ch === "(" || ch === "[" || ch === "{") depth += 1;
      else if (ch === ")" || ch === "]" || ch === "}") depth = Math.max(0, depth - 1);
      else if (ch === ";" && depth === 0) {
        flush();
      }
    }
  }
  flush();
  return out;
}

function extractEquationBlocks(source: string): string[] {
  const lines = source.split(/\r?\n/);
  const out: string[] = [];
  let inEquation = false;
  let inInitialEquation = false;
  for (const raw of lines) {
    const line = raw.trim();
    const lower = line.toLowerCase();
    if (lower === "equation") {
      inEquation = true;
      inInitialEquation = false;
      continue;
    }
    if (lower === "initial equation") {
      inEquation = false;
      inInitialEquation = true;
      continue;
    }
    if (
      lower === "algorithm" ||
      lower === "initial algorithm" ||
      lower.startsWith("end ")
    ) {
      inEquation = false;
      inInitialEquation = false;
      continue;
    }
    if (!inEquation || inInitialEquation) {
      continue;
    }
    if (!line || line.startsWith("//")) {
      continue;
    }
    out.push(line);
  }
  return splitEquationStatements(out);
}

function buildEquationSignatures(source: string): EquationSignature[] {
  const blocks = extractEquationBlocks(source);
  return blocks.map((b, idx) => {
    const canonical = normalizeEquationTokens(b);
    return {
      index: idx,
      hash: fnv1a32(canonical),
    };
  });
}

function buildChangedEquationKeys(
  prevCode: string | null,
  nextCode: string
): EquationGraphNodeKey[] | undefined {
  if (prevCode === null || prevCode === nextCode) {
    return undefined;
  }
  const prev = buildEquationSignatures(prevCode);
  const next = buildEquationSignatures(nextCode);
  const maxLen = Math.max(prev.length, next.length);
  const changed: EquationGraphNodeKey[] = [];
  for (let i = 0; i < maxLen; i += 1) {
    if ((prev[i]?.hash ?? -1) !== (next[i]?.hash ?? -1)) {
      const hash = next[i]?.hash ?? 0;
      changed.push({ Equation: { index: i, hash } });
    }
  }
  return changed.length > 0 ? changed : undefined;
}

type GraphNodeData = {
  label: string;
  width: number;
  height: number;
  kind: string;
};

type EqgraphMetricsBadge = {
  latestSkipRatio: number;
  averageSkipRatio: number;
  sampleCount: number;
};

const EQGRAPH_METRICS_SAMPLE_N = 12;
const EQGRAPH_METRICS_SESSION_PREFIX = "eqgraph-view";

function createMetricsSessionId(
  modelName: string,
  graphMode: EquationGraphMode,
  projectDir: string | null | undefined
): string {
  const modelKey = modelName
    .toLowerCase()
    .replace(/[^a-z0-9_.-]+/g, "_")
    .slice(0, 48);
  const projectNorm = (projectDir ?? "none").toLowerCase();
  const projectKey = fnv1a32(projectNorm).toString(16).padStart(8, "0");
  return `${EQGRAPH_METRICS_SESSION_PREFIX}-${projectKey}-${modelKey}-${graphMode}-${Math.random()
    .toString(36)
    .slice(2, 8)}`;
}

function parseSkipRatioFromMetricsMessage(message: string): number | null {
  const m = message.match(/skip_ratio=([0-9]*\.?[0-9]+)/);
  if (!m) return null;
  const n = Number.parseFloat(m[1]);
  return Number.isFinite(n) ? n : null;
}

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
  graphMode?: EquationGraphMode;
  onGraphModeChange?: (mode: EquationGraphMode) => void;
  dependencyGraphBehavior?: Partial<DependencyGraphBehavior>;
  onReady?: (handle: JointPaperHandle | null) => void;
}

export function EquationGraphView({
  code,
  modelName,
  projectDir,
  layoutOptions: externalLayout,
  graphMode = "compact",
  onGraphModeChange,
  dependencyGraphBehavior: behaviorProp,
  onReady,
}: EquationGraphViewProps) {
  const downgradeTargetResolved: "compact" | "top-level" =
    behaviorProp?.downgradeTarget === "top-level"
      ? "top-level"
      : behaviorProp?.downgradeTarget === "compact"
        ? "compact"
        : DEFAULT_DEPENDENCY_GRAPH_BEHAVIOR.downgradeTarget;
  const fullTimeoutResolved =
    behaviorProp?.fullTimeoutSec !== undefined
      ? behaviorProp.fullTimeoutSec
      : DEFAULT_DEPENDENCY_GRAPH_BEHAVIOR.fullTimeoutSec;
  const behavior: DependencyGraphBehavior = {
    fullTimeoutSec: Math.min(300, Math.max(1, fullTimeoutResolved)),
    autoDowngradeFromFull:
      behaviorProp?.autoDowngradeFromFull ?? DEFAULT_DEPENDENCY_GRAPH_BEHAVIOR.autoDowngradeFromFull,
    downgradeTarget: downgradeTargetResolved,
    initialGraphMode:
      behaviorProp?.initialGraphMode ?? DEFAULT_DEPENDENCY_GRAPH_BEHAVIOR.initialGraphMode,
  };
  const alternateGraphMode: EquationGraphMode =
    behavior.downgradeTarget === "compact" ? "top-level" : "compact";
  const [graph, setGraph] = useState<EquationGraph | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMessage, setLoadingMessage] = useState("Loading equation graph...");
  const [autoDowngradeReason, setAutoDowngradeReason] = useState<string | null>(null);
  const [metricsBadge, setMetricsBadge] = useState<EqgraphMetricsBadge | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const graphRef = useRef<dia.Graph | null>(null);
  const paperRef = useRef<dia.Paper | null>(null);
  const initializedRef = useRef(false);
  const requestSeqRef = useRef(0);
  const layoutOptions = externalLayout ?? DEFAULT_LAYOUT;
  const { schemeId } = useDiagramScheme();
  const previousCodeRef = useRef<string | null>(null);
  const metricsSessionIdRef = useRef<string>(
    createMetricsSessionId(modelName, graphMode, projectDir)
  );
  const showDevMetrics = Boolean(import.meta.env.DEV);

  useEffect(() => {
    metricsSessionIdRef.current = createMetricsSessionId(modelName, graphMode, projectDir);
    setMetricsBadge(null);
  }, [modelName, graphMode, projectDir]);

  useEffect(() => {
    let cancelled = false;
    let beat: ReturnType<typeof setInterval> | null = null;
    let fullFallbackTimer: ReturnType<typeof setTimeout> | null = null;
    let activeMode: EquationGraphMode = graphMode;
    let started = Date.now();

    async function loadGraph(mode: EquationGraphMode) {
      const currentSeq = ++requestSeqRef.current;
      activeMode = mode;
      started = Date.now();
      setLoading(true);
      setError(null);
      setAutoDowngradeReason(null);
      setLoadingMessage("Loading equation graph...");
      if (beat) clearInterval(beat);
      beat = setInterval(() => {
        if (cancelled) return;
        const sec = Math.max(1, Math.floor((Date.now() - started) / 1000));
        setLoadingMessage(`Loading equation graph (${sec}s)...`);
      }, 5000);
      try {
        const changedKeys = buildChangedEquationKeys(previousCodeRef.current, code);
        const graphResult = await getEquationGraphV2(
          code,
          modelName,
          projectDir,
          mode,
          changedKeys,
          metricsSessionIdRef.current
        );
        if (cancelled || currentSeq !== requestSeqRef.current) return;
        if (!graphResult.ok || !graphResult.data) {
          const errText = graphResult.errors.map((e) => `${e.code}: ${e.message}`).join("; ");
          throw new Error(errText || "Equation graph build failed");
        }
        setGraph(graphResult.data);
        previousCodeRef.current = code;
        if (showDevMetrics) {
          const records = await getMonitorEvents(metricsSessionIdRef.current, 300);
          if (!cancelled && currentSeq === requestSeqRef.current) {
            const ratios = records
              .filter(
                (r) =>
                  r.task === "equation-graph" &&
                  r.stage === "metrics" &&
                  r.sessionId === metricsSessionIdRef.current
              )
              .map((r) => parseSkipRatioFromMetricsMessage(r.message))
              .filter((v): v is number => v !== null)
              .slice(-EQGRAPH_METRICS_SAMPLE_N);
            if (ratios.length > 0) {
              const latest = ratios[ratios.length - 1];
              const avg = ratios.reduce((a, b) => a + b, 0) / ratios.length;
              setMetricsBadge({
                latestSkipRatio: latest,
                averageSkipRatio: avg,
                sampleCount: ratios.length,
              });
            } else {
              setMetricsBadge(null);
            }
          }
        }
        if (fullFallbackTimer) {
          clearTimeout(fullFallbackTimer);
          fullFallbackTimer = null;
        }
      } catch (loadError) {
        if (!cancelled && currentSeq === requestSeqRef.current) {
          setError(String(loadError));
        }
      } finally {
        if (!cancelled && currentSeq === requestSeqRef.current) {
          if (beat) {
            clearInterval(beat);
            beat = null;
          }
          setLoading(false);
        }
      }
    }

    if (graphMode === "full" && behavior.autoDowngradeFromFull && behavior.fullTimeoutSec >= 1) {
      fullFallbackTimer = setTimeout(() => {
        if (cancelled) return;
        if (activeMode !== "full") return;
        const target = behavior.downgradeTarget;
        const modeLabel =
          target === "top-level" ? t("dependencyGraphModeTopLevel") : t("dependencyGraphModeCompact");
        setAutoDowngradeReason(
          tf("dependencyGraphAutoDowngraded", { seconds: String(behavior.fullTimeoutSec), mode: modeLabel })
        );
        onGraphModeChange?.(target);
        void loadGraph(target);
      }, behavior.fullTimeoutSec * 1000);
    } else if (
      (graphMode === "compact" || graphMode === "top-level") &&
      behavior.autoDowngradeFromFull &&
      behavior.fullTimeoutSec >= 1
    ) {
      // Compact / top-level both require a full flatten + inline pipeline
      // server-side. For models that pull in large MSL subtrees (e.g.
      // ComponentLibraryCoverage, which transitively loads dozens of
      // Modelica.Blocks / Mechanics / Thermal classes from a 2500+ file
      // MSL) the cold flatten can take many seconds, leaving the user
      // staring at "Loading equation graph...". Fall back to the
      // structural mode (parses declarations + connect equations only,
      // no flatten) so something is always shown within the configured
      // timeout window.
      fullFallbackTimer = setTimeout(() => {
        if (cancelled) return;
        if (activeMode !== "compact" && activeMode !== "top-level") return;
        setAutoDowngradeReason(
          tf("dependencyGraphAutoDowngraded", {
            seconds: String(behavior.fullTimeoutSec),
            mode: t("dependencyGraphModeStructural"),
          })
        );
        onGraphModeChange?.("structural");
        void loadGraph("structural");
      }, behavior.fullTimeoutSec * 1000);
    }

    // Debounce the heavy equation-graph rebuild so rapid source edits
    // (typing or diagram drag/drop) don't pile up uncancellable blocking
    // tasks on the Tauri thread pool. The previous behaviour kicked off a
    // new build on every keystroke which made the IDE feel hung for
    // complex models.
    const loadDebounceMs = 700;
    setLoading(true);
    setLoadingMessage("Loading equation graph...");
    const loadDelayTimer = setTimeout(() => {
      if (cancelled) return;
      void loadGraph(graphMode);
    }, loadDebounceMs);

    return () => {
      cancelled = true;
      if (beat) clearInterval(beat);
      if (fullFallbackTimer) clearTimeout(fullFallbackTimer);
      clearTimeout(loadDelayTimer);
    };
  }, [
    code,
    modelName,
    projectDir,
    graphMode,
    onGraphModeChange,
    behavior.autoDowngradeFromFull,
    behavior.downgradeTarget,
    behavior.fullTimeoutSec,
  ]);

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
      const isInstance = node.data.kind === "instance";
      const isConnector = node.data.kind === "connector";
      const isComponent = node.data.kind === "component" || isInstance;
      const nodeFill = isEquation
        ? equationFill
        : isComponent
          ? colorToRgba(theme.primary, 0.22)
          : isConnector
            ? colorToRgba(theme.border, 0.28)
            : variableFill;
      const el = new shapes.standard.Rectangle({
        id: node.id,
        position: { x: node.x, y: node.y },
        size: { width: node.data.width, height: node.data.height },
        attrs: {
          body: {
            rx: 4,
            ry: 4,
            fill: nodeFill,
            stroke: isEquation
              ? theme.primary
              : isComponent
                ? theme.primary
                : isConnector
                  ? theme.border
                  : theme.border,
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
        {loadingMessage}
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
      {autoDowngradeReason ? (
        <div className="absolute top-2 left-2 z-10 rounded border border-blue-500/40 bg-blue-500/15 px-2 py-1 text-[10px] text-blue-100 flex items-center gap-2">
          <span>{autoDowngradeReason}</span>
          <button
            type="button"
            className="rounded border border-blue-300/40 px-1 py-0.5 hover:bg-blue-400/20"
            onClick={() => onGraphModeChange?.(alternateGraphMode)}
          >
            {alternateGraphMode === "top-level" ? t("dependencyGraphSwitchToTopLevel") : t("dependencyGraphSwitchToCompact")}
          </button>
        </div>
      ) : null}
      {showDevMetrics && metricsBadge ? (
        <div
          className={`absolute left-2 z-10 rounded border border-emerald-500/40 bg-emerald-500/15 px-2 py-1 text-[10px] text-emerald-100 ${
            autoDowngradeReason ? "top-10" : "top-2"
          }`}
        >
          {`eqgraph skip ratio avg(${metricsBadge.sampleCount})=${metricsBadge.averageSkipRatio.toFixed(3)} latest=${metricsBadge.latestSkipRatio.toFixed(3)}`}
        </div>
      ) : null}
      {graph?.truncated ? (
        <div className="absolute top-2 right-2 z-10 rounded border border-amber-500/40 bg-amber-500/15 px-2 py-1 text-[10px] text-amber-200 flex items-center gap-2">
          <span>
            {tf("dependencyGraphTruncatedHint", {
              included: String(graph.includedEquations ?? 0),
              total: String(graph.totalEquations ?? 0),
            })}
          </span>
          <button
            type="button"
            className="rounded border border-amber-300/40 px-1 py-0.5 hover:bg-amber-400/20"
            onClick={() => onGraphModeChange?.(alternateGraphMode)}
          >
            {alternateGraphMode === "top-level" ? t("dependencyGraphSwitchToTopLevel") : t("dependencyGraphSwitchToCompact")}
          </button>
        </div>
      ) : null}
      <div ref={containerRef} className="absolute inset-0" />
    </div>
  );
}
