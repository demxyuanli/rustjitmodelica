import React, { useCallback, useEffect, useRef, useState } from "react";
import { dia } from "@joint/core";
import { useDiagramScheme } from "../../contexts/DiagramSchemeContext";

interface JointMiniMapProps {
  paper: dia.Paper | null;
  graph: dia.Graph | null;
  width?: number;
  height?: number;
}

interface MiniRect {
  x: number;
  y: number;
  w: number;
  h: number;
  isCircle: boolean;
}

interface MiniLine {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

interface MinimapState {
  elements: MiniRect[];
  links: MiniLine[];
  viewport: { x: number; y: number; w: number; h: number };
  modelBounds: { x: number; y: number; w: number; h: number };
  minimapScale: number;
  offsetX: number;
  offsetY: number;
}

function computeModelBounds(graph: dia.Graph) {
  const els = graph.getElements();
  if (els.length === 0) return null;

  let minX = Infinity,
    minY = Infinity,
    maxX = -Infinity,
    maxY = -Infinity;

  for (const el of els) {
    const pos = el.position();
    const size = el.size();
    minX = Math.min(minX, pos.x);
    minY = Math.min(minY, pos.y);
    maxX = Math.max(maxX, pos.x + size.width);
    maxY = Math.max(maxY, pos.y + size.height);
  }

  const pad = 60;
  return {
    x: minX - pad,
    y: minY - pad,
    w: maxX - minX + pad * 2,
    h: maxY - minY + pad * 2,
  };
}

export const JointMiniMap = React.memo(function JointMiniMap({
  paper,
  graph,
  width = 180,
  height = 120,
}: JointMiniMapProps) {
  const [state, setState] = useState<MinimapState | null>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const isDragging = useRef(false);
  const dragStart = useRef({ x: 0, y: 0, tx: 0, ty: 0 });
  const { scheme } = useDiagramScheme();
  const viewportStroke = scheme.diagramPrimary ?? "var(--primary)";
  const viewportFill = scheme.diagramPrimary ? `${scheme.diagramPrimary}14` : "rgba(59,130,246,0.08)";

  const computeState = useCallback(() => {
    if (!paper || !graph) return;

    const bounds = computeModelBounds(graph);
    if (!bounds || bounds.w <= 0 || bounds.h <= 0) return;

    const scaleX = width / bounds.w;
    const scaleY = height / bounds.h;
    const minimapScale = Math.min(scaleX, scaleY);

    const drawnW = bounds.w * minimapScale;
    const drawnH = bounds.h * minimapScale;
    const offsetX = (width - drawnW) / 2;
    const offsetY = (height - drawnH) / 2;

    const toMini = (mx: number, my: number) => ({
      x: (mx - bounds.x) * minimapScale + offsetX,
      y: (my - bounds.y) * minimapScale + offsetY,
    });

    const elements: MiniRect[] = graph.getElements().map((el) => {
      const pos = el.position();
      const size = el.size();
      const mp = toMini(pos.x, pos.y);
      return {
        x: mp.x,
        y: mp.y,
        w: size.width * minimapScale,
        h: size.height * minimapScale,
        isCircle: (el.get("type") as string) === "standard.Circle",
      };
    });

    const links: MiniLine[] = graph.getLinks().map((link) => {
      const srcEl = link.source().id ? graph.getCell(link.source().id!) : null;
      const tgtEl = link.target().id ? graph.getCell(link.target().id!) : null;
      const sc = srcEl?.isElement()
        ? (() => {
            const p = (srcEl as dia.Element).position();
            const s = (srcEl as dia.Element).size();
            return { x: p.x + s.width / 2, y: p.y + s.height / 2 };
          })()
        : { x: 0, y: 0 };
      const tc = tgtEl?.isElement()
        ? (() => {
            const p = (tgtEl as dia.Element).position();
            const s = (tgtEl as dia.Element).size();
            return { x: p.x + s.width / 2, y: p.y + s.height / 2 };
          })()
        : { x: 0, y: 0 };
      const sp = toMini(sc.x, sc.y);
      const tp = toMini(tc.x, tc.y);
      return { x1: sp.x, y1: sp.y, x2: tp.x, y2: tp.y };
    });

    const paperScale = paper.scale().sx;
    const t = paper.translate();
    const pSize = paper.getComputedSize();

    const vpModelX = -t.tx / paperScale;
    const vpModelY = -t.ty / paperScale;
    const vpModelW = pSize.width / paperScale;
    const vpModelH = pSize.height / paperScale;

    const vpMini = toMini(vpModelX, vpModelY);

    setState({
      elements,
      links,
      viewport: {
        x: vpMini.x,
        y: vpMini.y,
        w: vpModelW * minimapScale,
        h: vpModelH * minimapScale,
      },
      modelBounds: bounds,
      minimapScale,
      offsetX,
      offsetY,
    });
  }, [paper, graph, width, height]);

  useEffect(() => {
    if (!paper || !graph) return;

    computeState();
    const tid = setInterval(computeState, 250);

    graph.on("change", computeState);
    graph.on("add", computeState);
    graph.on("remove", computeState);

    return () => {
      clearInterval(tid);
      graph.off("change", computeState);
      graph.off("add", computeState);
      graph.off("remove", computeState);
    };
  }, [paper, graph, computeState]);

  const handlePointerDown = useCallback(
    (evt: React.PointerEvent) => {
      if (!paper || !state) return;
      evt.preventDefault();
      isDragging.current = true;
      dragStart.current = {
        x: evt.clientX,
        y: evt.clientY,
        tx: paper.translate().tx,
        ty: paper.translate().ty,
      };
      (evt.target as Element).setPointerCapture(evt.pointerId);
    },
    [paper, state]
  );

  const handlePointerMove = useCallback(
    (evt: React.PointerEvent) => {
      if (!isDragging.current || !paper || !state) return;

      const dx = evt.clientX - dragStart.current.x;
      const dy = evt.clientY - dragStart.current.y;

      const modelDx = dx / state.minimapScale;
      const modelDy = dy / state.minimapScale;
      const paperScale = paper.scale().sx;

      paper.translate(
        dragStart.current.tx - modelDx * paperScale,
        dragStart.current.ty - modelDy * paperScale
      );
      computeState();
    },
    [paper, state, computeState]
  );

  const handlePointerUp = useCallback(() => {
    isDragging.current = false;
  }, []);

  if (!state || state.elements.length === 0) return null;

  return (
    <div
      className="absolute bottom-2 right-2 z-10 rounded border border-[var(--border)] bg-[var(--bg)]/80 backdrop-blur-sm shadow-lg overflow-hidden"
      style={{ width, height }}
    >
      <svg
        ref={svgRef}
        width={width}
        height={height}
        className="block"
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        style={{ cursor: isDragging.current ? "grabbing" : "grab" }}
      >
        {state.links.map((l, i) => (
          <line
            key={`l${i}`}
            x1={l.x1}
            y1={l.y1}
            x2={l.x2}
            y2={l.y2}
            stroke="var(--text-muted)"
            strokeWidth={0.5}
            opacity={0.5}
          />
        ))}
        {state.elements.map((el, i) => (
          <rect
            key={`e${i}`}
            x={el.x}
            y={el.y}
            width={Math.max(2, el.w)}
            height={Math.max(2, el.h)}
            rx={el.isCircle ? 999 : 1}
            fill="var(--bg-elevated)"
            stroke="var(--border)"
            strokeWidth={0.5}
          />
        ))}
        <rect
          x={state.viewport.x}
          y={state.viewport.y}
          width={Math.max(1, state.viewport.w)}
          height={Math.max(1, state.viewport.h)}
          fill={viewportFill}
          stroke={viewportStroke}
          strokeWidth={1}
          rx={1}
        />
      </svg>
    </div>
  );
});
