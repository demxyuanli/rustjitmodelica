import React from "react";
import type { AnnotationColor } from "./diagramGraphicTypes";

function colorToCSS(c?: AnnotationColor): string {
  if (!c) return "currentColor";
  return `rgb(${c.r},${c.g},${c.b})`;
}

export function createBSplinePath(points: { x: number; y: number }[]): string {
  if (points.length < 2) return "";
  if (points.length === 2) {
    return `M ${points[0].x} ${points[0].y} L ${points[1].x} ${points[1].y}`;
  }

  let d = `M ${points[0].x} ${points[0].y}`;

  for (let i = 0; i < points.length - 1; i++) {
    const p0 = points[i];
    const p1 = points[i + 1];

    const tension = 1 / 6;

    let cp1x: number;
    let cp1y: number;
    let cp2x: number;
    let cp2y: number;

    if (i === 0) {
      const pPrev = { x: 2 * p0.x - p1.x, y: 2 * p0.y - p1.y };
      cp1x = p0.x + (p1.x - pPrev.x) * tension;
      cp1y = p0.y + (p1.y - pPrev.y) * tension;
    } else {
      const pPrev = points[i - 1];
      cp1x = p0.x + (p1.x - pPrev.x) * tension;
      cp1y = p0.y + (p1.y - pPrev.y) * tension;
    }

    if (i === points.length - 2) {
      const pNext = { x: 2 * p1.x - p0.x, y: 2 * p1.y - p0.y };
      cp2x = p1.x - (pNext.x - p0.x) * tension;
      cp2y = p1.y - (pNext.y - p0.y) * tension;
    } else {
      const pNext = points[i + 2];
      cp2x = p1.x - (pNext.x - p0.x) * tension;
      cp2y = p1.y - (pNext.y - p0.y) * tension;
    }

    d += ` C ${cp1x.toFixed(2)} ${cp1y.toFixed(2)}, ${cp2x.toFixed(2)} ${cp2y.toFixed(2)}, ${p1.x.toFixed(2)} ${p1.y.toFixed(2)}`;
  }

  return d;
}

export function renderArrowheads(
  points: { x: number; y: number }[],
  arrowSpec: string[],
  color?: AnnotationColor,
  arrowSize?: number,
): React.ReactNode {
  if (points.length < 2) return null;

  const size = arrowSize ?? 10;
  const lastIdx = points.length - 1;
  const arrows: React.ReactNode[] = [];

  const hasStartArrow = arrowSpec.some((a) => a.toLowerCase() === "start" || a.toLowerCase() === "first");
  const hasEndArrow =
    arrowSpec.some((a) => a.toLowerCase() === "end" || a.toLowerCase() === "last") || arrowSpec.length === 0;

  const arrowType: "filled" | "open" | "arrow" =
    (arrowSpec.find((a) => ["filled", "open", "arrow"].includes(a.toLowerCase()))?.toLowerCase() as
      | "filled"
      | "open"
      | "arrow") || "filled";

  if (hasEndArrow) {
    const p1 = points[lastIdx - 1];
    const p2 = points[lastIdx];
    const angle = Math.atan2(p2.y - p1.y, p2.x - p1.x);
    arrows.push(renderArrowhead(p2, angle, arrowType, size, color, `arrow-end-${lastIdx}`));
  }

  if (hasStartArrow) {
    const p0 = points[0];
    const p1 = points[1];
    const angle = Math.atan2(p1.y - p0.y, p1.x - p0.x);
    arrows.push(renderArrowhead(p0, angle, arrowType, size, color, "arrow-start-0"));
  }

  return <>{arrows}</>;
}

function renderArrowhead(
  point: { x: number; y: number },
  angle: number,
  type: "filled" | "open" | "arrow" = "filled",
  size: number = 10,
  color?: AnnotationColor,
  key?: string,
): React.ReactNode {
  const colorStr = colorToCSS(color);
  const halfAngle = Math.PI / 6;

  switch (type) {
    case "filled":
    case "arrow": {
      const cos1 = Math.cos(angle - halfAngle);
      const sin1 = Math.sin(angle - halfAngle);
      const cos2 = Math.cos(angle + halfAngle);
      const sin2 = Math.sin(angle + halfAngle);

      const polyPoints = [
        `${point.x},${point.y}`,
        `${point.x - size * cos1},${point.y - size * sin1}`,
        `${point.x - size * cos2},${point.y - size * sin2}`,
      ].join(" ");

      return (
        <polygon
          key={key}
          points={polyPoints}
          fill={colorStr}
          stroke={colorStr}
          strokeWidth={1}
        />
      );
    }
    case "open": {
      const cos1 = Math.cos(angle - halfAngle);
      const sin1 = Math.sin(angle - halfAngle);
      const cos2 = Math.cos(angle + halfAngle);
      const sin2 = Math.sin(angle + halfAngle);

      const p1: { x: number; y: number } = {
        x: point.x - size * cos1,
        y: point.y - size * sin1,
      };
      const p2: { x: number; y: number } = {
        x: point.x - size * cos2,
        y: point.y - size * sin2,
      };
      return (
        <path
          key={key}
          d={`M ${p1.x} ${p1.y} L ${point.x} ${point.y} L ${p2.x} ${p2.y}`}
          stroke={colorStr}
          strokeWidth={1}
          fill="none"
        />
      );
    }
    default:
      return null;
  }
}
