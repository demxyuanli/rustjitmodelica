import { useCallback, useMemo, useRef, useState } from "react";
import {
  Background,
  BackgroundVariant,
  ConnectionLineType,
  ConnectionMode,
  Controls,
  MarkerType,
  MiniMap,
  ReactFlow,
  type Connection,
  type EdgeChange,
  type NodeChange,
  type OnConnectStartParams,
  type OnSelectionChangeParams,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { Clock, Map as MapIcon, Spline } from "lucide-react";
import type { ReactFlowInstance } from "@xyflow/react";
import type { CatalogEntry, DiagramDocument } from "../api/types";
import { localizeConnectReason } from "../advisor/format";
import { t } from "../i18n";
import { readCatalogDragId } from "../library/catalogDnd";
import { CanvasContextMenu } from "./CanvasContextMenu";
import { placeCatalogNode } from "./placeCatalog";
import { StudioBlockNode, type StudioFlowNode } from "./StudioBlockNode";
import { StudioDrawNode } from "./StudioDrawNode";
import { StudioStepEdge, type StudioFlowEdge } from "./StudioStepEdge";
import {
  branchFanOut,
  isBranchedEdge,
  shortEdgeLabel,
  shouldShowBranchLabel,
} from "./edgeLayout";
import { isDrawNode } from "./drawParams";
import {
  canConnect,
  compatibleMagnetPorts,
  findNode,
  portOf,
  upsertEdge,
  type ConnectingPort,
} from "./connectionRules";
import { CanvasEdgeDock, type CanvasEdgeDockId } from "./CanvasEdgeDock";
import { SampleTimeLegend } from "./SampleTimeLegend";
import { DomainEdgeLegend } from "./DomainEdgeLegend";
import {
  edgeDisplayLabel,
  edgeDomainOf,
  edgeTooltip,
  edgeVisual,
} from "./edgeStyle";
import { GRID, snapSize, snapToGrid, validateEdges } from "./editOps";
import { inferPortDimension, inferPortUnit } from "./sensorMeta";
import { rerouteEdgesTouchingNodes, routeEdgeVertices } from "./edgePath";

const nodeTypes = { studio: StudioBlockNode, studioDraw: StudioDrawNode };
const edgeTypes = { studioStep: StudioStepEdge };
const EMPTY_POS: Record<string, { x: number; y: number }> = {};

function sameIdList(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((id, i) => id === b[i]);
}

function toFlow(
  diagram: DiagramDocument,
  opts?: { edgeLabelsVisible?: boolean; edgeLabelOffsetY?: number },
): {
  nodes: StudioFlowNode[];
  edges: StudioFlowEdge[];
} {
  const labelsOn = opts?.edgeLabelsVisible !== false;
  const labelOffY = opts?.edgeLabelOffsetY ?? -10;
  const validity = new Map(validateEdges(diagram).map((v) => [v.id, v]));
  const fanOut = branchFanOut(diagram);
  return {
    nodes: diagram.nodes.map((n) => {
      const draw = isDrawNode(n);
      const w = n.ui?.width ?? (draw ? 140 : undefined);
      const h = n.ui?.height ?? (draw ? 80 : undefined);
      return {
        id: n.id,
        type: (draw ? "studioDraw" : "studio") as "studio" | "studioDraw",
        position: n.position,
        data: { diagram: n },
        selected: false,
        zIndex: draw ? -1 : 0,
        style: draw && w != null && h != null ? { width: w, height: h } : undefined,
      };
    }),
    edges: diagram.edges.map((e) => {
      const v = validity.get(e.id);
      const domain = edgeDomainOf(diagram, e);
      const bad = !!(v && !v.ok);
      const vis = edgeVisual(domain, bad);
      const src = findNode(diagram, e.sourceNodeId);
      const label = edgeDisplayLabel(e.signalName, e.unit, shortEdgeLabel);
      const branched = isBranchedEdge(e, fanOut);
      const showLabel =
        labelsOn &&
        (shouldShowBranchLabel(e, fanOut, diagram.edges) ||
          !!e.unit?.trim() ||
          !!e.signalName?.trim());
      const tip = edgeTooltip(e, domain, src);
      return {
        id: e.id,
        type: "studioStep" as const,
        source: e.sourceNodeId,
        target: e.targetNodeId,
        sourceHandle: e.sourcePortId,
        targetHandle: e.targetPortId,
        label,
        data: {
          branched,
          showLabel,
          showJunction: branched && showLabel,
          fullLabel: tip,
          vertices: e.vertices ?? [],
          domain,
          unit: e.unit ?? undefined,
          dimension: e.dimension ?? undefined,
          labelOffsetY: labelOffY,
        },
        className: `slx-edge domain-${domain}${bad ? " is-bad" : ""}`,
        style: {
          stroke: vis.stroke,
          strokeWidth: vis.strokeWidth,
          ...(vis.strokeDasharray ? { strokeDasharray: vis.strokeDasharray } : {}),
        },
        markerEnd: vis.marker
          ? {
              type: MarkerType.ArrowClosed,
              width: 14,
              height: 14,
              color: vis.stroke,
            }
          : undefined,
        animated: bad,
      };
    }),
  };
}

export interface StudioCanvasProps {
  diagram: DiagramDocument;
  diagramRev: number;
  selectedIds: string[];
  selectedEdgeIds?: string[];
  snapEnabled: boolean;
  onDiagramChange: (d: DiagramDocument) => void;
  onSelectionChange: (nodeIds: string[], edgeIds: string[]) => void;
  catalog?: CatalogEntry[];
  pendingCatalog: CatalogEntry | null;
  onConsumePendingCatalog: () => void;
  focusNodeId?: string | null;
  onNodeDoubleClick?: (nodeId: string) => void;
  onConnectRejected?: (reason: string) => void;
  onCreateSubsystem?: () => void;
  onPlotSelection?: () => void;
  onRotateSelection?: () => void;
  onDeleteSelection?: () => void;
  /** F52 Display/Scope end values keyed by node id. */
  displayValues?: Record<string, string>;
  /** When false, hide edge signal labels (settings). */
  edgeLabelsVisible?: boolean;
  /** Vertical label offset in px (settings). */
  edgeLabelOffsetY?: number;
}

export function StudioCanvas({
  diagram,
  diagramRev,
  selectedIds,
  selectedEdgeIds = [],
  snapEnabled,
  onDiagramChange,
  onSelectionChange,
  catalog = [],
  pendingCatalog,
  onConsumePendingCatalog,
  focusNodeId,
  onNodeDoubleClick,
  onConnectRejected,
  onCreateSubsystem,
  onPlotSelection,
  onRotateSelection,
  onDeleteSelection,
  displayValues = {},
  edgeLabelsVisible = true,
  edgeLabelOffsetY = -10,
}: StudioCanvasProps) {
  const rfRef = useRef<ReactFlowInstance<StudioFlowNode, StudioFlowEdge> | null>(null);
  const [connectHint, setConnectHint] = useState<string | null>(null);
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number } | null>(null);
  const [connecting, setConnecting] = useState<ConnectingPort | null>(null);
  const [pinnedDocks, setPinnedDocks] = useState<Partial<Record<CanvasEdgeDockId, boolean>>>({});
  const [dragPos, setDragPos] = useState<Record<string, { x: number; y: number }>>({});
  const dragRev = useRef(diagramRev);
  const livePos = dragRev.current === diagramRev ? dragPos : EMPTY_POS;

  const flowSize = (fn: StudioFlowNode): { width?: number; height?: number } => {
    const sw = fn.style?.width;
    const sh = fn.style?.height;
    const width =
      typeof sw === "number"
        ? sw
        : typeof sw === "string"
          ? Number.parseFloat(sw)
          : typeof fn.width === "number"
            ? fn.width
            : undefined;
    const height =
      typeof sh === "number"
        ? sh
        : typeof sh === "string"
          ? Number.parseFloat(sh)
          : typeof fn.height === "number"
            ? fn.height
            : undefined;
    return {
      width: Number.isFinite(width) ? width : undefined,
      height: Number.isFinite(height) ? height : undefined,
    };
  };

  const documentFromFlow = useCallback(
    (nextNodes: StudioFlowNode[], nextEdges: StudioFlowEdge[]): DiagramDocument => {
      const byId = new Map(diagram.nodes.map((n) => [n.id, n]));
      const prevEdge = new Map(diagram.edges.map((e) => [e.id, e]));
      const nodesOut = nextNodes.map((fn) => {
        const prev = byId.get(fn.id) ?? fn.data.diagram;
        const pos = snapEnabled
          ? snapToGrid(fn.position.x, fn.position.y)
          : { x: fn.position.x, y: fn.position.y };
        const size = isDrawNode(prev) || isDrawNode(fn.data.diagram) ? flowSize(fn) : {};
        return {
          ...prev,
          ...fn.data.diagram,
          id: fn.id,
          position: pos,
          ui: {
            ...prev.ui,
            ...fn.data.diagram.ui,
            ...(size.width != null ? { width: size.width } : {}),
            ...(size.height != null ? { height: size.height } : {}),
          },
        };
      });
      return {
        ...diagram,
        nodes: nodesOut,
        edges: nextEdges.map((e) => {
          const prev = prevEdge.get(e.id);
          return {
            id: e.id,
            sourceNodeId: e.source,
            targetNodeId: e.target,
            sourcePortId: String(e.sourceHandle ?? ""),
            targetPortId: String(e.targetHandle ?? ""),
            signalName: prev?.signalName ?? null,
            unit: prev?.unit ?? null,
            dimension: prev?.dimension ?? null,
            vertices: e.data?.vertices ?? prev?.vertices ?? [],
            routing: "orthogonal" as const,
          };
        }),
      };
    },
    [diagram, snapEnabled],
  );

  const commitNodeDrag = useCallback(
    (nextNodes: StudioFlowNode[], nextEdges: StudioFlowEdge[]) => {
      const next = documentFromFlow(nextNodes, nextEdges);
      const prevById = new Map(diagram.nodes.map((n) => [n.id, n]));
      const movedIds: string[] = [];
      for (const n of next.nodes) {
        const prev = prevById.get(n.id);
        if (!prev) continue;
        if (
          Math.abs(prev.position.x - n.position.x) > 0.5 ||
          Math.abs(prev.position.y - n.position.y) > 0.5
        ) {
          movedIds.push(n.id);
        }
      }
      if (!movedIds.length) return;
      onDiagramChange(rerouteEdgesTouchingNodes(next, movedIds));
    },
    [diagram, documentFromFlow, onDiagramChange],
  );

  const onVerticesChange = useCallback(
    (edgeId: string, vertices: { x: number; y: number }[]) => {
      onDiagramChange({
        ...diagram,
        edges: diagram.edges.map((e) =>
          e.id === edgeId ? { ...e, vertices } : e,
        ),
      });
    },
    [diagram, onDiagramChange],
  );
  const onVerticesChangeRef = useRef(onVerticesChange);
  onVerticesChangeRef.current = onVerticesChange;
  const onVerts = useCallback((id: string, verts: { x: number; y: number }[]) => {
    onVerticesChangeRef.current(id, verts);
  }, []);

  const flowBase = useMemo(
    () => toFlow(diagram, { edgeLabelsVisible, edgeLabelOffsetY }),
    [diagram, edgeLabelsVisible, edgeLabelOffsetY],
  );

  const magnetByNode = useMemo(
    () => (connecting ? compatibleMagnetPorts(diagram, connecting) : null),
    [connecting, diagram],
  );

  const nodes: StudioFlowNode[] = useMemo(
    () =>
      flowBase.nodes.map((n) => ({
        ...n,
        position: livePos[n.id] ?? n.position,
        selected: selectedIds.includes(n.id) || n.id === focusNodeId,
        data: {
          ...n.data,
          displayValue: displayValues[n.id],
          magnetPortIds: magnetByNode?.get(n.id),
        },
      })),
    [flowBase.nodes, livePos, selectedIds, focusNodeId, displayValues, magnetByNode],
  );

  const edges: StudioFlowEdge[] = useMemo(
    () =>
      flowBase.edges.map((e) => ({
        ...e,
        selected: selectedEdgeIds.includes(e.id),
        data: { ...e.data, onVerticesChange: onVerts },
      })),
    [flowBase.edges, selectedEdgeIds, onVerts],
  );

  const handleNodesChange = useCallback(
    (chs: NodeChange<StudioFlowNode>[]) => {
      const mapped = chs.map((c) => {
        if (
          c.type === "dimensions" &&
          c.dimensions &&
          snapEnabled &&
          c.resizing === false
        ) {
          return {
            ...c,
            dimensions: {
              width: snapSize(c.dimensions.width, GRID, GRID),
              height: snapSize(c.dimensions.height, GRID, 8),
            },
          };
        }
        return c;
      });

      const posUpdates: Record<string, { x: number; y: number }> = {};
      let hasPos = false;
      for (const c of mapped) {
        if (c.type === "position" && c.position) {
          posUpdates[c.id] = c.position;
          hasPos = true;
        }
      }
      if (hasPos) {
        setDragPos((prev) => {
          const base = dragRev.current === diagramRev ? prev : EMPTY_POS;
          let changed = false;
          const next: Record<string, { x: number; y: number }> = { ...base };
          for (const [id, pos] of Object.entries(posUpdates)) {
            if (next[id]?.x === pos.x && next[id]?.y === pos.y) continue;
            next[id] = pos;
            changed = true;
          }
          if (!changed) return prev;
          dragRev.current = diagramRev;
          return next;
        });
      }

      const ended = mapped.filter(
        (c): c is NodeChange<StudioFlowNode> & {
          type: "dimensions";
          dimensions: { width: number; height: number };
        } =>
          c.type === "dimensions" &&
          c.resizing === false &&
          !!c.dimensions &&
          typeof c.dimensions.width === "number" &&
          typeof c.dimensions.height === "number",
      );
      if (!ended.length) return;

      let changed = false;
      const nodesOut = diagram.nodes.map((n) => {
        if (!isDrawNode(n)) return n;
        const ch = ended.find((c) => c.id === n.id);
        if (!ch) return n;
        const width = ch.dimensions.width;
        const height = ch.dimensions.height;
        if (n.ui?.width === width && n.ui?.height === height) return n;
        changed = true;
        return { ...n, ui: { ...n.ui, width, height } };
      });
      if (changed) onDiagramChange({ ...diagram, nodes: nodesOut });
    },
    [diagram, diagramRev, onDiagramChange, snapEnabled],
  );

  const onConnect = useCallback(
    (c: Connection) => {
      setConnecting(null);
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
        const reason = localizeConnectReason(decision.reason);
        setConnectHint(reason);
        onConnectRejected?.(reason);
        return;
      }
      setConnectHint(null);
      const unit =
        inferPortUnit(sn, c.sourceHandle) ?? inferPortUnit(tn, c.targetHandle);
      const dimension =
        inferPortDimension(sn, c.sourceHandle) ??
        inferPortDimension(tn, c.targetHandle);
      const edge = {
        id: `e_${Date.now()}`,
        sourceNodeId: c.source,
        sourcePortId: c.sourceHandle,
        targetNodeId: c.target,
        targetPortId: c.targetHandle,
        signalName: `${sn.instanceName}.${c.sourceHandle}`,
        unit,
        dimension,
        vertices: routeEdgeVertices(diagram, {
          sourceNodeId: c.source,
          sourcePortId: c.sourceHandle,
          targetNodeId: c.target,
          targetPortId: c.targetHandle,
        }),
        routing: "orthogonal" as const,
      };
      onDiagramChange(upsertEdge(diagram, edge));
    },
    [diagram, onDiagramChange, onConnectRejected],
  );

  const onConnectStart = useCallback((_e: unknown, params: OnConnectStartParams) => {
    if (!params.nodeId) return;
    setConnecting({
      nodeId: params.nodeId,
      handleId: params.handleId ?? null,
      handleType: params.handleType === "target" ? "target" : "source",
    });
  }, []);

  const onConnectEnd = useCallback(() => {
    setConnecting(null);
  }, []);

  const flowPosFromClient = useCallback(
    (clientX: number, clientY: number) => {
      const mapped = rfRef.current?.screenToFlowPosition({ x: clientX, y: clientY });
      if (mapped) return mapped;
      return { x: clientX, y: clientY };
    },
    [],
  );

  const placeEntry = useCallback(
    (entry: CatalogEntry, clientX: number, clientY: number) => {
      const placed = placeCatalogNode(entry, flowPosFromClient(clientX, clientY), diagram, snapEnabled);
      onDiagramChange(placed.next);
      onConsumePendingCatalog();
      setConnectHint(null);
      setCtxMenu(null);
    },
    [diagram, flowPosFromClient, onConsumePendingCatalog, onDiagramChange, snapEnabled],
  );

  const onPaneClick = useCallback(
    (ev: React.MouseEvent) => {
      setCtxMenu(null);
      setPinnedDocks({});
      if (!pendingCatalog) return;
      placeEntry(pendingCatalog, ev.clientX, ev.clientY);
    },
    [pendingCatalog, placeEntry],
  );

  const setDockPinned = useCallback((id: CanvasEdgeDockId, pinned: boolean) => {
    setPinnedDocks((prev) => ({ ...prev, [id]: pinned }));
  }, []);

  const onDragOverPane = useCallback((ev: React.DragEvent) => {
    if (!readCatalogDragId(ev.dataTransfer) && !ev.dataTransfer.types.includes("text/plain")) return;
    ev.preventDefault();
    ev.dataTransfer.dropEffect = "copy";
  }, []);

  const onDropPane = useCallback(
    (ev: React.DragEvent) => {
      ev.preventDefault();
      const id = readCatalogDragId(ev.dataTransfer);
      if (!id) return;
      const entry = catalog.find((c) => c.id === id) ?? (pendingCatalog?.id === id ? pendingCatalog : null);
      if (!entry) return;
      placeEntry(entry, ev.clientX, ev.clientY);
    },
    [catalog, pendingCatalog, placeEntry],
  );

  const lastSel = useRef({ n: selectedIds, e: selectedEdgeIds });
  lastSel.current = { n: selectedIds, e: selectedEdgeIds };
  const onSel = useCallback(
    (p: OnSelectionChangeParams) => {
      const n = p.nodes.map((node) => node.id);
      const e = p.edges.map((edge) => edge.id);
      if (sameIdList(lastSel.current.n, n) && sameIdList(lastSel.current.e, e)) return;
      lastSel.current = { n, e };
      onSelectionChange(n, e);
    },
    [onSelectionChange],
  );

  const invalidCount = useMemo(
    () => validateEdges(diagram).filter((v) => !v.ok).length,
    [diagram],
  );

  const openCtx = (ev: { preventDefault: () => void; clientX: number; clientY: number }) => {
    ev.preventDefault();
    setCtxMenu({ x: ev.clientX, y: ev.clientY });
  };

  const bannerText = connectHint
    ? connectHint
    : invalidCount > 0
      ? `${invalidCount} ${t("invalidConnections")}`
      : null;

  return (
    <div
      className="canvas-wrap slx-canvas"
      style={{ width: "100%", height: "100%" }}
      onDragOver={onDragOverPane}
      onDrop={onDropPane}
    >
      {bannerText ? <div className="canvas-banner">{bannerText}</div> : null}
      {ctxMenu ? (
        <CanvasContextMenu
          x={ctxMenu.x}
          y={ctxMenu.y}
          canPlace={!!pendingCatalog}
          canSubsystem={selectedIds.length >= 1}
          canPlot={selectedEdgeIds.length >= 1}
          canRotate={selectedIds.length >= 1}
          canDelete={selectedIds.length >= 1 || selectedEdgeIds.length >= 1}
          onAction={(a) => {
            if (a === "place" && pendingCatalog) {
              placeEntry(pendingCatalog, ctxMenu.x, ctxMenu.y);
              return;
            }
            if (a === "subsystem") onCreateSubsystem?.();
            if (a === "plot") onPlotSelection?.();
            if (a === "rotate") onRotateSelection?.();
            if (a === "delete") onDeleteSelection?.();
          }}
          onClose={() => setCtxMenu(null)}
        />
      ) : null}
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onInit={(inst) => {
          rfRef.current = inst;
        }}
        onNodesChange={handleNodesChange}
        onEdgesChange={(chs: EdgeChange[]) => {
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
        onConnectStart={onConnectStart}
        onConnectEnd={onConnectEnd}
        connectionRadius={40}
        onDragOver={onDragOverPane}
        onDrop={onDropPane}
        onPaneClick={onPaneClick}
        onPaneContextMenu={openCtx}
        onNodeContextMenu={openCtx}
        onEdgeContextMenu={openCtx}
        onSelectionChange={onSel}
        onNodeDragStop={(_e, _n, ns) => commitNodeDrag(ns as StudioFlowNode[], edges)}
        onNodeDoubleClick={(_e, node) => onNodeDoubleClick?.(node.id)}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        defaultEdgeOptions={{
          type: "studioStep",
        }}
        connectionLineType={ConnectionLineType.Step}
        connectionMode={ConnectionMode.Loose}
        multiSelectionKeyCode={["Control", "Meta"]}
        selectionKeyCode="Shift"
        fitView
        snapToGrid={snapEnabled}
        snapGrid={[10, 10]}
        deleteKeyCode={["Backspace", "Delete"]}
        proOptions={{ hideAttribution: true }}
        onNodesDelete={(deleted) => {
          const ids = new Set(deleted.map((n) => n.id));
          onDiagramChange({
            ...diagram,
            nodes: diagram.nodes.filter((n) => !ids.has(n.id)),
            edges: diagram.edges.filter((e) => !ids.has(e.sourceNodeId) && !ids.has(e.targetNodeId)),
          });
        }}
      >
        <Background
          id="slx-grid"
          variant={BackgroundVariant.Lines}
          gap={20}
          color="var(--canvas-grid)"
        />
        <Background
          id="slx-grid-major"
          variant={BackgroundVariant.Lines}
          gap={100}
          color="color-mix(in srgb, var(--canvas-grid) 70%, var(--text))"
        />
        <Controls showInteractive={false} />
        <CanvasEdgeDock
          id="wires"
          side="left"
          label={t("domainEdgeLegend")}
          icon={Spline}
          pinned={!!pinnedDocks.wires}
          onPinnedChange={(pinned) => setDockPinned("wires", pinned)}
        >
          <DomainEdgeLegend />
        </CanvasEdgeDock>
        <CanvasEdgeDock
          id="sampleTime"
          side="left"
          label={t("sampleTimeLegend")}
          icon={Clock}
          pinned={!!pinnedDocks.sampleTime}
          onPinnedChange={(pinned) => setDockPinned("sampleTime", pinned)}
        >
          <SampleTimeLegend />
        </CanvasEdgeDock>
        <CanvasEdgeDock
          id="minimap"
          side="right"
          label={t("canvasMinimap")}
          icon={MapIcon}
          pinned={!!pinnedDocks.minimap}
          onPinnedChange={(pinned) => setDockPinned("minimap", pinned)}
        >
          <MiniMap
            nodeStrokeWidth={2}
            nodeColor={(n) => {
              const d = (n.data as {
                diagram?: { domain?: string; nodeKind?: string; catalogId?: string };
              } | undefined)?.diagram;
              const cid = (d?.catalogId ?? "").toLowerCase();
              if (cid.includes("inport") || cid.includes("terminals.in")) return "var(--inport)";
              if (cid.includes("outport") || cid.includes("terminals.out")) return "var(--outport)";
              if (cid.includes("routing.goto")) return "var(--goto)";
              if (cid.includes("routing.from")) return "var(--from)";
              if (cid.includes("routing.") || cid.includes("mux") || cid.includes("demux") || cid.includes("bus")) {
                return "var(--busbar)";
              }
              if (d?.nodeKind === "subsystem") return "var(--block-sub-border)";
              if (d?.nodeKind === "fmu") return "var(--block-fmu-border)";
              if (d?.nodeKind === "draw" || d?.domain === "draw") return "var(--accent)";
              if (d?.domain === "signal") return "var(--signal)";
              if (d?.domain === "electrical") return "var(--connector)";
              if (d?.domain === "mechanical") return "var(--ok)";
              if (d?.domain === "multibody") return "var(--ok)";
              return "var(--muted)";
            }}
            maskColor="color-mix(in srgb, var(--bg) 55%, transparent)"
          />
        </CanvasEdgeDock>
      </ReactFlow>
    </div>
  );
}
