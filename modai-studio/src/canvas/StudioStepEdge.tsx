import {
  BaseEdge,
  EdgeLabelRenderer,
  getSmoothStepPath,
  Position,
  useReactFlow,
  type Edge,
  type EdgeProps,
} from "@xyflow/react";
import { useCallback, useRef, type MouseEvent, type PointerEvent } from "react";
import type { Position as XY } from "../api/types";
import {
  defaultVertex,
  edgeCenter,
  orthogonalize,
  pathFromPoints,
} from "./edgePath";

export type StudioEdgeData = {
  branched?: boolean;
  showLabel?: boolean;
  showJunction?: boolean;
  fullLabel?: string;
  vertices?: XY[];
  onVerticesChange?: (edgeId: string, vertices: XY[]) => void;
  domain?: string;
  unit?: string;
  dimension?: string;
  labelOffsetY?: number;
};

export type StudioFlowEdge = Edge<StudioEdgeData, "studioStep">;

function snap10(n: number): number {
  return Math.round(n / 10) * 10;
}

export function StudioStepEdge({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  style,
  markerEnd,
  label,
  data,
  selected,
}: EdgeProps<StudioFlowEdge>) {
  const { screenToFlowPosition } = useReactFlow();
  const verts = data?.vertices ?? [];
  const onVerticesChange = data?.onVerticesChange;
  const dragIdx = useRef<number | null>(null);

  let edgePath: string;
  let labelX: number;
  let labelY: number;

  if (verts.length > 0) {
    const pts = orthogonalize([
      { x: sourceX, y: sourceY },
      ...verts,
      { x: targetX, y: targetY },
    ]);
    edgePath = pathFromPoints(pts);
    const c = edgeCenter(pts);
    labelX = c.x;
    labelY = c.y;
  } else {
    [edgePath, labelX, labelY] = getSmoothStepPath({
      sourceX,
      sourceY,
      sourcePosition,
      targetX,
      targetY,
      targetPosition,
      borderRadius: 0,
      offset: 16,
    });
  }

  const stroke =
    typeof style?.stroke === "string" ? style.stroke : "var(--signal)";
  const showJunction = !!data?.showJunction;
  const showLabel = data?.showLabel !== false && !!label;
  const branched = !!data?.branched;

  let jx = sourceX;
  let jy = sourceY;
  if (sourcePosition === Position.Right) jx = sourceX + 16;
  else if (sourcePosition === Position.Left) jx = sourceX - 16;
  else if (sourcePosition === Position.Bottom) jy = sourceY + 16;
  else if (sourcePosition === Position.Top) jy = sourceY - 16;

  let lx = labelX;
  let ly = labelY + (data?.labelOffsetY ?? -10);
  if (branched && showLabel) {
    if (sourcePosition === Position.Right) {
      lx = jx + 18;
      ly = jy - 12;
    } else if (sourcePosition === Position.Left) {
      lx = jx - 18;
      ly = jy - 12;
    } else if (sourcePosition === Position.Bottom) {
      lx = jx + 14;
      ly = jy + 4;
    } else {
      lx = jx + 14;
      ly = jy - 4;
    }
  }

  const onVertexPointerDown = useCallback(
    (idx: number, ev: PointerEvent) => {
      if (!onVerticesChange) return;
      ev.preventDefault();
      ev.stopPropagation();
      dragIdx.current = idx;
      (ev.currentTarget as HTMLElement).setPointerCapture(ev.pointerId);
    },
    [onVerticesChange],
  );

  const onVertexPointerMove = useCallback(
    (ev: PointerEvent) => {
      if (dragIdx.current == null || !onVerticesChange) return;
      const p = screenToFlowPosition({ x: ev.clientX, y: ev.clientY });
      const idx = dragIdx.current;
      const next = verts.map((v, i) =>
        i === idx ? { x: snap10(p.x), y: snap10(p.y) } : v,
      );
      onVerticesChange(id, next);
    },
    [id, onVerticesChange, screenToFlowPosition, verts],
  );

  const onVertexPointerUp = useCallback((ev: PointerEvent) => {
    dragIdx.current = null;
    try {
      (ev.currentTarget as HTMLElement).releasePointerCapture(ev.pointerId);
    } catch {
      /* ignore */
    }
  }, []);

  const addVertex = useCallback(
    (ev: MouseEvent) => {
      if (!onVerticesChange) return;
      if (!(ev.altKey || ev.detail === 2)) return;
      ev.stopPropagation();
      const mid =
        verts.length === 0
          ? defaultVertex({ x: sourceX, y: sourceY }, { x: targetX, y: targetY })
          : defaultVertex(verts[verts.length - 1]!, { x: targetX, y: targetY });
      onVerticesChange(id, [...verts, mid]);
    },
    [id, onVerticesChange, sourceX, sourceY, targetX, targetY, verts],
  );

  const removeVertex = useCallback(
    (idx: number, ev: MouseEvent) => {
      if (!onVerticesChange) return;
      ev.stopPropagation();
      onVerticesChange(
        id,
        verts.filter((_, i) => i !== idx),
      );
    },
    [id, onVerticesChange, verts],
  );

  return (
    <>
      <BaseEdge id={id} path={edgePath} style={style} markerEnd={markerEnd} />
      <path
        d={edgePath}
        fill="none"
        stroke="transparent"
        strokeWidth={18}
        className="slx-edge-hit"
        onDoubleClick={addVertex}
        onClick={addVertex}
      />
      {showJunction ? (
        <circle
          className="slx-junction"
          cx={jx}
          cy={jy}
          r={3.5}
          fill={stroke}
          stroke="var(--panel)"
          strokeWidth={1}
        />
      ) : null}
      {showLabel ? (
        <EdgeLabelRenderer>
          <div
            className="slx-edge-label nodrag nopan"
            title={data?.fullLabel ?? String(label)}
            style={{
              position: "absolute",
              transform: `translate(-50%, -50%) translate(${lx}px, ${ly}px)`,
              pointerEvents: "all",
            }}
          >
            {label}
          </div>
        </EdgeLabelRenderer>
      ) : null}
      {selected && onVerticesChange ? (
        <EdgeLabelRenderer>
          {verts.map((v, i) => (
            <div
              key={`v${i}`}
              className="slx-edge-vertex nodrag nopan"
              title="Drag; double-click remove; Alt+click / double-click wire to add"
              style={{
                position: "absolute",
                transform: `translate(-50%, -50%) translate(${v.x}px, ${v.y}px)`,
                pointerEvents: "all",
              }}
              onPointerDown={(ev) => onVertexPointerDown(i, ev)}
              onPointerMove={onVertexPointerMove}
              onPointerUp={onVertexPointerUp}
              onDoubleClick={(ev) => removeVertex(i, ev)}
            />
          ))}
        </EdgeLabelRenderer>
      ) : null}
    </>
  );
}
