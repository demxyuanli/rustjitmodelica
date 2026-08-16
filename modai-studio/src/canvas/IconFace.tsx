import type { ReactNode } from "react";

/** Lightweight Modelica Icon annotation → SVG (Rust serde snake_case shape). */

export type AnnPoint = { x: number; y: number };
export type AnnExtent = { p1: AnnPoint; p2: AnnPoint };
export type AnnColor = { r: number; g: number; b: number };

export type IconGraphic = {
  type: string;
  points?: AnnPoint[];
  extent?: AnnExtent;
  color?: AnnColor;
  line_color?: AnnColor;
  lineColor?: AnnColor;
  fill_color?: AnnColor;
  fillColor?: AnnColor;
  fill_pattern?: string;
  fillPattern?: string;
  border_pattern?: string;
  borderPattern?: string;
  line_thickness?: number;
  lineThickness?: number;
  thickness?: number;
  pattern?: string;
  radius?: number;
  text_string?: string;
  textString?: string;
  text_color?: AnnColor;
  textColor?: AnnColor;
  font_size?: number;
  fontSize?: number;
  layerHidden?: boolean;
  layer_hidden?: boolean;
  children?: IconGraphic[];
};

export type IconAnnotation = {
  coordinate_system?: { extent?: AnnExtent };
  coordinateSystem?: { extent?: AnnExtent };
  graphics?: IconGraphic[];
};

const DEFAULT_EXT: AnnExtent = { p1: { x: -100, y: -100 }, p2: { x: 100, y: 100 } };

function csExtent(icon: IconAnnotation): AnnExtent {
  return (
    icon.coordinate_system?.extent ??
    icon.coordinateSystem?.extent ??
    DEFAULT_EXT
  );
}

function colorCss(c?: AnnColor, fallback = "var(--text)"): string {
  if (!c) return fallback;
  return `rgb(${c.r},${c.g},${c.b})`;
}

function isFilled(pattern?: string): boolean {
  if (!pattern) return true;
  const p = pattern.toLowerCase();
  return p !== "none" && p !== "fillpattern.none";
}

function dash(pattern?: string): string | undefined {
  if (!pattern) return undefined;
  const p = pattern.toLowerCase();
  if (p.includes("dash") && p.includes("dot")) return "6 3 1 3";
  if (p.includes("dash")) return "6 4";
  if (p.includes("dot")) return "1 3";
  return undefined;
}

function toSvg(
  p: AnnPoint,
  ext: AnnExtent,
  w: number,
  h: number,
): { x: number; y: number } {
  const cw = Math.abs(ext.p2.x - ext.p1.x) || 200;
  const ch = Math.abs(ext.p2.y - ext.p1.y) || 200;
  const minX = Math.min(ext.p1.x, ext.p2.x);
  const maxY = Math.max(ext.p1.y, ext.p2.y);
  return {
    x: ((p.x - minX) / cw) * w,
    y: ((maxY - p.y) / ch) * h,
  };
}

function extentRect(ext: AnnExtent, cs: AnnExtent, w: number, h: number) {
  const a = toSvg(ext.p1, cs, w, h);
  const b = toSvg(ext.p2, cs, w, h);
  return {
    x: Math.min(a.x, b.x),
    y: Math.min(a.y, b.y),
    width: Math.max(1, Math.abs(b.x - a.x)),
    height: Math.max(1, Math.abs(b.y - a.y)),
  };
}

function renderItem(
  g: IconGraphic,
  key: string,
  cs: AnnExtent,
  w: number,
  h: number,
  instanceName?: string,
): ReactNode {
  if (g.layerHidden || g.layer_hidden) return null;
  const lineColor = g.line_color ?? g.lineColor ?? g.color;
  const fillColor = g.fill_color ?? g.fillColor;
  const fillPattern = g.fill_pattern ?? g.fillPattern;
  const borderPattern = g.border_pattern ?? g.borderPattern ?? g.pattern;
  const strokeW = g.line_thickness ?? g.lineThickness ?? g.thickness ?? 1;

  switch (g.type) {
    case "Line": {
      const pts = (g.points ?? []).map((p) => toSvg(p, cs, w, h));
      if (pts.length < 2) return null;
      const d = pts.map((p, i) => `${i === 0 ? "M" : "L"} ${p.x} ${p.y}`).join(" ");
      return (
        <path
          key={key}
          d={d}
          fill="none"
          stroke={colorCss(lineColor)}
          strokeWidth={strokeW}
          strokeDasharray={dash(g.pattern)}
        />
      );
    }
    case "Rectangle": {
      if (!g.extent) return null;
      const r = extentRect(g.extent, cs, w, h);
      return (
        <rect
          key={key}
          x={r.x}
          y={r.y}
          width={r.width}
          height={r.height}
          rx={g.radius ?? 0}
          fill={isFilled(fillPattern) ? colorCss(fillColor, "transparent") : "none"}
          stroke={colorCss(lineColor)}
          strokeWidth={strokeW}
          strokeDasharray={dash(borderPattern)}
        />
      );
    }
    case "Ellipse": {
      if (!g.extent) return null;
      const r = extentRect(g.extent, cs, w, h);
      return (
        <ellipse
          key={key}
          cx={r.x + r.width / 2}
          cy={r.y + r.height / 2}
          rx={r.width / 2}
          ry={r.height / 2}
          fill={isFilled(fillPattern) ? colorCss(fillColor, "transparent") : "none"}
          stroke={colorCss(lineColor)}
          strokeWidth={strokeW}
          strokeDasharray={dash(borderPattern)}
        />
      );
    }
    case "Polygon": {
      const pts = (g.points ?? []).map((p) => toSvg(p, cs, w, h));
      if (pts.length < 3) return null;
      return (
        <polygon
          key={key}
          points={pts.map((p) => `${p.x},${p.y}`).join(" ")}
          fill={isFilled(fillPattern) ? colorCss(fillColor, "transparent") : "none"}
          stroke={colorCss(lineColor)}
          strokeWidth={strokeW}
          strokeDasharray={dash(borderPattern)}
        />
      );
    }
    case "Text": {
      if (!g.extent) return null;
      const r = extentRect(g.extent, cs, w, h);
      let text = g.text_string ?? g.textString ?? "";
      if (instanceName) text = text.replace(/%name/gi, instanceName);
      text = text.replace(/%[a-zA-Z.]+/g, "");
      const tc = g.text_color ?? g.textColor ?? lineColor;
      const fs = g.font_size ?? g.fontSize ?? Math.max(8, r.height * 0.45);
      return (
        <text
          key={key}
          x={r.x + r.width / 2}
          y={r.y + r.height / 2}
          textAnchor="middle"
          dominantBaseline="middle"
          fill={colorCss(tc)}
          fontSize={fs}
          fontFamily="var(--studio-font)"
        >
          {text}
        </text>
      );
    }
    case "Group": {
      return (
        <g key={key}>
          {(g.children ?? []).map((c, i) => renderItem(c, `${key}.${i}`, cs, w, h, instanceName))}
        </g>
      );
    }
    default:
      return null;
  }
}

export function IconFace({
  icon,
  width,
  height,
  instanceName,
}: {
  icon: IconAnnotation;
  width: number;
  height: number;
  instanceName?: string;
}) {
  const graphics = icon.graphics ?? [];
  if (!graphics.length) return null;
  const cs = csExtent(icon);
  const w = Math.max(8, width);
  const h = Math.max(8, height);
  return (
    <svg className="slx-icon-face" width={w} height={h} viewBox={`0 0 ${w} ${h}`} aria-hidden>
      {graphics.map((g, i) => renderItem(g, String(i), cs, w, h, instanceName))}
    </svg>
  );
}

export function asIconAnnotation(raw: unknown): IconAnnotation | null {
  if (!raw || typeof raw !== "object") return null;
  const o = raw as IconAnnotation;
  if (!Array.isArray(o.graphics) || o.graphics.length === 0) return null;
  return o;
}
