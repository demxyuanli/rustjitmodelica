import { NodeResizer, type NodeProps } from "@xyflow/react";
import { dashArray, drawShape, paramOf } from "./drawParams";
import type { StudioFlowNode } from "./StudioBlockNode";

export function StudioDrawNode({
  data,
  selected,
  width: measuredW,
  height: measuredH,
}: NodeProps<StudioFlowNode>) {
  const n = data.diagram;
  const shape = drawShape(n);
  const width = measuredW ?? n.ui?.width ?? 140;
  const height = measuredH ?? n.ui?.height ?? 80;
  const fill = paramOf(n, "fill", "transparent");
  const stroke = paramOf(n, "stroke", "var(--accent)");
  const strokeWidth = Number(paramOf(n, "strokeWidth", "1.5")) || 1.5;
  const pattern = paramOf(n, "linePattern", "solid");
  const dash = dashArray(pattern, strokeWidth);
  const text = paramOf(n, "text", n.ui?.label ?? n.instanceName);
  const fontSize = Number(paramOf(n, "fontSize", "12")) || 12;
  const minH = shape === "line" ? 8 : shape === "text" ? 16 : 24;
  const minW = shape === "line" ? 40 : 32;

  return (
    <>
      <NodeResizer
        isVisible={selected}
        minWidth={minW}
        minHeight={minH}
        color="var(--accent)"
        handleClassName="slx-resize-handle"
        lineClassName="slx-resize-line"
      />
      <div
        className={`slx-draw shape-${shape}${selected ? " selected" : ""}`}
        style={{ width: "100%", height: "100%" }}
      >
        {shape === "text" ? (
          <div className="slx-draw-text" style={{ color: stroke, fontSize }} title={text}>
            {text}
          </div>
        ) : shape === "note" ? (
          <div
            className="slx-draw-note"
            style={{
              background: fill,
              borderColor: stroke,
              borderWidth: strokeWidth,
              fontSize,
              color: "var(--text)",
            }}
          >
            {text}
          </div>
        ) : (
          <svg
            className="slx-draw-svg"
            width="100%"
            height="100%"
            viewBox={`0 0 ${width} ${height}`}
            preserveAspectRatio="none"
          >
            {shape === "rectangle" ? (
              <rect
                x={strokeWidth}
                y={strokeWidth}
                width={Math.max(0, width - strokeWidth * 2)}
                height={Math.max(0, height - strokeWidth * 2)}
                fill={fill}
                stroke={stroke}
                strokeWidth={strokeWidth}
                strokeDasharray={dash}
              />
            ) : null}
            {shape === "ellipse" ? (
              <ellipse
                cx={width / 2}
                cy={height / 2}
                rx={Math.max(0, width / 2 - strokeWidth)}
                ry={Math.max(0, height / 2 - strokeWidth)}
                fill={fill}
                stroke={stroke}
                strokeWidth={strokeWidth}
                strokeDasharray={dash}
              />
            ) : null}
            {shape === "line" ? (
              <line
                x1={2}
                y1={height / 2}
                x2={width - 2}
                y2={height / 2}
                stroke={stroke}
                strokeWidth={strokeWidth}
                strokeDasharray={dash}
                strokeLinecap="square"
              />
            ) : null}
            {shape === "polygon" ? (
              <polygon
                points={`${width / 2},${strokeWidth} ${width - strokeWidth},${height - strokeWidth} ${strokeWidth},${height - strokeWidth}`}
                fill={fill}
                stroke={stroke}
                strokeWidth={strokeWidth}
                strokeDasharray={dash}
                strokeLinejoin="miter"
              />
            ) : null}
          </svg>
        )}
      </div>
    </>
  );
}
