import type { AnnotationColor } from "../diagramGraphicTypes";

export function colorToCSS(c?: AnnotationColor): string {
  if (!c) return "currentColor";
  return `rgb(${c.r},${c.g},${c.b})`;
}

/** Map Modelica-style line/border pattern names to SVG stroke-dasharray. */
export function patternStringToStrokeDasharray(pattern?: string): string | undefined {
  if (!pattern) return undefined;
  const norm = pattern
    .toLowerCase()
    .replace(/\s+/g, "")
    .replace(/linepattern\./g, "")
    .replace(/borderpattern\./g, "");
  if (norm.includes("dotdashed") || norm.includes("dashdot")) return "2 4 8 4";
  if (norm.includes("dotted")) return "2 4";
  if (norm.includes("dashed")) return "8 4";
  if (norm.includes("solid") || norm === "none") return undefined;
  return undefined;
}
