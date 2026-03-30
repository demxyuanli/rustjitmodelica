/**
 * Graphic export utilities
 * Provides functions to export graphics as SVG and PNG
 */

import type {
  AnnotationColor,
  AnnotationPoint,
  CoordinateSystem,
  GraphicItem,
  IconDiagramAnnotation,
} from "../components/diagramGraphicTypes";
import { graphicOuterTransformSvg, patternStringToStrokeDasharray } from "../components/DiagramSvgRenderer";

export interface ExportOptions {
  width: number;
  height: number;
  backgroundColor?: string;
  scale?: number;
}

const DEFAULT_EXPORT_OPTIONS: ExportOptions = {
  width: 800,
  height: 600,
  backgroundColor: "#ffffff",
  scale: 2,
};

const PNG_EXPORT_TIMEOUT_MS = 20_000;

function escapeXmlText(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function escapeXmlAttr(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/"/g, "&quot;");
}

function colorToSvgString(c?: AnnotationColor): string {
  if (!c) return "currentColor";
  return `rgb(${c.r},${c.g},${c.b})`;
}

function isFilledExport(pattern?: string): boolean {
  if (!pattern) return false;
  const lower = pattern.toLowerCase();
  if (lower === "none") return false;
  return (
    lower.includes("solid") ||
    lower.includes("horizontal") ||
    lower.includes("vertical") ||
    lower.includes("cross") ||
    lower.includes("diagcross") ||
    lower.includes("forward") ||
    lower.includes("backward")
  );
}

function gradientDefsString(graphics: GraphicItem[]): string {
  const parts: string[] = [];
  function walk(items: GraphicItem[], prefix: string) {
    items.forEach((item, i) => {
      const path = prefix === "" ? `${i}` : `${prefix}-${i}`;
      if (item.type === "Group") {
        walk(item.children, path);
      } else if ("fillGradient" in item && item.fillGradient) {
        const id = `gradient-${path}`;
        if (item.fillGradient.type === "linearGradient") {
          const g = item.fillGradient.gradient;
          const stops = g.stops
            .map(
              (s) =>
                `<stop offset="${s.offset * 100}%" stop-color="${colorToSvgString(s.color)}" stop-opacity="${s.opacity ?? 1}"/>`,
            )
            .join("");
          parts.push(
            `<linearGradient id="${escapeXmlAttr(id)}" x1="${g.x1}" y1="${g.y1}" x2="${g.x2}" y2="${g.y2}">${stops}</linearGradient>`,
          );
        } else {
          const g = item.fillGradient.gradient;
          const stops = g.stops
            .map(
              (s) =>
                `<stop offset="${s.offset * 100}%" stop-color="${colorToSvgString(s.color)}" stop-opacity="${s.opacity ?? 1}"/>`,
            )
            .join("");
          parts.push(
            `<radialGradient id="${escapeXmlAttr(id)}" cx="${g.cx}" cy="${g.cy}" r="${g.r}">${stops}</radialGradient>`,
          );
        }
      }
    });
  }
  walk(graphics, "");
  return parts.join("");
}

function wrapItemSvg(
  inner: string,
  item: GraphicItem,
  extent: { p1: AnnotationPoint; p2: AnnotationPoint },
  svgWidth: number,
  svgHeight: number,
): string {
  if (!inner) return "";
  const cs: CoordinateSystem = { extent };
  const tf = graphicOuterTransformSvg(item, cs, svgWidth, svgHeight);
  const op = item.opacity != null && !Number.isNaN(item.opacity) ? Math.max(0, Math.min(1, item.opacity)) : 1;
  const ta = tf ? ` transform="${escapeXmlAttr(tf)}"` : "";
  const oa = op < 1 ? ` opacity="${op}"` : "";
  if (!ta && !oa) return inner;
  return `<g${ta}${oa}>${inner}</g>`;
}

/**
 * Export graphics as SVG string
 */
export function exportToSvg(
  annotation: IconDiagramAnnotation,
  options: ExportOptions = DEFAULT_EXPORT_OPTIONS,
): string {
  const { width, height, backgroundColor = "#ffffff" } = options;
  const extent = annotation.coordinateSystem?.extent ?? {
    p1: { x: -100, y: -100 },
    p2: { x: 100, y: 100 },
  };

  const defs = gradientDefsString(annotation.graphics);
  const defsBlock = defs ? `\n  <defs>${defs}</defs>` : "";

  const graphicsSvg = annotation.graphics
    .map((item, idx) => renderGraphicItemToSvg(item, String(idx), extent, width, height))
    .join("\n");

  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" 
     width="${width}" 
     height="${height}" 
     viewBox="0 0 ${width} ${height}">
  <rect width="100%" height="100%" fill="${backgroundColor}"/>${defsBlock}
  ${graphicsSvg}
</svg>`;
}

/**
 * Export graphics as PNG data URL
 */
export async function exportToPng(
  annotation: IconDiagramAnnotation,
  options: ExportOptions = DEFAULT_EXPORT_OPTIONS,
): Promise<string> {
  const svgString = exportToSvg(annotation, options);
  const dataUrlSvg = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svgString)}`;

  const loadWithSrc = (src: string, revokeObjectUrl?: string) =>
    new Promise<HTMLImageElement>((resolve, reject) => {
      const img = new Image();
      const timeout = window.setTimeout(() => {
        if (revokeObjectUrl) URL.revokeObjectURL(revokeObjectUrl);
        reject(new Error("PNG export timed out"));
      }, PNG_EXPORT_TIMEOUT_MS);
      img.onload = () => {
        window.clearTimeout(timeout);
        if (revokeObjectUrl) URL.revokeObjectURL(revokeObjectUrl);
        resolve(img);
      };
      img.onerror = () => {
        window.clearTimeout(timeout);
        if (revokeObjectUrl) URL.revokeObjectURL(revokeObjectUrl);
        reject(new Error("Failed to decode SVG for PNG export"));
      };
      img.src = src;
    });

  let img: HTMLImageElement;
  try {
    img = await loadWithSrc(dataUrlSvg);
  } catch {
    const svgBlob = new Blob([svgString], { type: "image/svg+xml;charset=utf-8" });
    const blobUrl = URL.createObjectURL(svgBlob);
    img = await loadWithSrc(blobUrl, blobUrl);
  }

  const canvas = document.createElement("canvas");
  const scale = options.scale ?? 2;
  canvas.width = options.width * scale;
  canvas.height = options.height * scale;
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("Could not get canvas context");
  }
  ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
  let png: string;
  try {
    png = canvas.toDataURL("image/png");
  } catch {
    throw new Error("Canvas is tainted or PNG encoding failed; try SVG export");
  }
  return png;
}

/**
 * Download SVG file
 */
export function downloadSvg(
  annotation: IconDiagramAnnotation,
  filename: string = "export.svg",
  options: ExportOptions = DEFAULT_EXPORT_OPTIONS,
): void {
  const svgString = exportToSvg(annotation, options);
  const blob = new Blob([svgString], { type: "image/svg+xml;charset=utf-8" });
  const url = URL.createObjectURL(blob);

  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
}

/**
 * Download PNG file
 */
export async function downloadPng(
  annotation: IconDiagramAnnotation,
  filename: string = "export.png",
  options: ExportOptions = DEFAULT_EXPORT_OPTIONS,
): Promise<void> {
  const dataUrl = await exportToPng(annotation, options);

  const link = document.createElement("a");
  link.href = dataUrl;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
}

/**
 * Render a single graphic item to SVG string
 */
function renderGraphicItemToSvg(
  item: GraphicItem,
  pathId: string,
  extent: { p1: AnnotationPoint; p2: AnnotationPoint },
  svgWidth: number,
  svgHeight: number,
): string {
  if (item.layerHidden) return "";
  const cw = Math.abs(extent.p2.x - extent.p1.x) || 200;
  const ch = Math.abs(extent.p2.y - extent.p1.y) || 200;
  const minX = Math.min(extent.p1.x, extent.p2.x);
  const maxY = Math.max(extent.p1.y, extent.p2.y);

  const coordToSvg = (p: AnnotationPoint) => ({
    x: ((p.x - minX) / cw) * svgWidth,
    y: ((maxY - p.y) / ch) * svgHeight,
  });

  const colorToSvg = (c?: AnnotationColor) => colorToSvgString(c);

  const dashAttr = (pattern?: string) => {
    const d = patternStringToStrokeDasharray(pattern);
    return d ? ` stroke-dasharray="${d}"` : "";
  };

  switch (item.type) {
    case "Line":
    case "BSpline": {
      if (item.points.length < 2) return "";
      const pts = item.points.map(coordToSvg);
      const d = pts.map((p, i) => `${i === 0 ? "M" : "L"} ${p.x} ${p.y}`).join(" ");
      const colorObj: AnnotationColor | undefined = item.color;
      const color = colorToSvg(colorObj);
      const thickness = item.thickness ?? 1;
      const inner = `<path d="${d}" stroke="${color}" stroke-width="${thickness}" fill="none"${dashAttr(item.pattern)}/>`;
      return wrapItemSvg(inner, item, extent, svgWidth, svgHeight);
    }

    case "Rectangle": {
      if (!item.extent) return "";
      const p1 = coordToSvg(item.extent.p1);
      const p2 = coordToSvg(item.extent.p2);
      const x = Math.min(p1.x, p2.x);
      const y = Math.min(p1.y, p2.y);
      const w = Math.abs(p2.x - p1.x);
      const h = Math.abs(p2.y - p1.y);
      const stroke = colorToSvg(item.lineColor);
      let fill: string;
      if (item.fillGradient) {
        fill = `url(#gradient-${pathId})`;
      } else if (isFilledExport(item.fillPattern)) {
        fill = colorToSvg(item.fillColor);
      } else {
        fill = "none";
      }
      const strokeWidth = item.lineThickness ?? 1;
      const rx = item.radius ?? 0;
      const inner = `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="${rx}" stroke="${stroke}" stroke-width="${strokeWidth}" fill="${fill}"${dashAttr(item.borderPattern)}/>`;
      return wrapItemSvg(inner, item, extent, svgWidth, svgHeight);
    }

    case "Ellipse": {
      if (!item.extent) return "";
      const p1 = coordToSvg(item.extent.p1);
      const p2 = coordToSvg(item.extent.p2);
      const cx = (p1.x + p2.x) / 2;
      const cy = (p1.y + p2.y) / 2;
      const rx = Math.abs(p2.x - p1.x) / 2;
      const ry = Math.abs(p2.y - p1.y) / 2;
      const stroke = colorToSvg(item.lineColor);
      let fill: string;
      if (item.fillGradient) {
        fill = `url(#gradient-${pathId})`;
      } else if (isFilledExport(item.fillPattern)) {
        fill = colorToSvg(item.fillColor);
      } else {
        fill = "none";
      }
      const strokeWidth = item.lineThickness ?? 1;
      const inner = `<ellipse cx="${cx}" cy="${cy}" rx="${rx}" ry="${ry}" stroke="${stroke}" stroke-width="${strokeWidth}" fill="${fill}"${dashAttr(item.linePattern)}/>`;
      return wrapItemSvg(inner, item, extent, svgWidth, svgHeight);
    }

    case "Polygon": {
      if (item.points.length < 2) return "";
      const pts = item.points.map(coordToSvg).map((p) => `${p.x},${p.y}`).join(" ");
      const stroke = colorToSvg(item.lineColor);
      let fill: string;
      if (item.fillGradient) {
        fill = `url(#gradient-${pathId})`;
      } else if (isFilledExport(item.fillPattern)) {
        fill = colorToSvg(item.fillColor);
      } else {
        fill = "none";
      }
      const strokeWidth = item.lineThickness ?? 1;
      const inner = `<polygon points="${pts}" stroke="${stroke}" stroke-width="${strokeWidth}" fill="${fill}"${dashAttr(item.linePattern)}/>`;
      return wrapItemSvg(inner, item, extent, svgWidth, svgHeight);
    }

    case "Text": {
      if (!item.extent) return "";
      const p1 = coordToSvg(item.extent.p1);
      const p2 = coordToSvg(item.extent.p2);
      const x = (p1.x + p2.x) / 2;
      const y = (p1.y + p2.y) / 2;
      const text = escapeXmlText(item.textString ?? "");
      const fontSize = item.fontSize ?? 12;
      const fill = colorToSvg(item.textColor ?? item.lineColor);
      const fontFamily = item.fontName || "sans-serif";
      const inner = `<text x="${x}" y="${y}" text-anchor="middle" dominant-baseline="central" font-size="${fontSize}" fill="${fill}" font-family="${escapeXmlAttr(fontFamily)}">${text}</text>`;
      return wrapItemSvg(inner, item, extent, svgWidth, svgHeight);
    }

    case "Bitmap": {
      if (!item.extent || !item.fileName) return "";
      const p1 = coordToSvg(item.extent.p1);
      const p2 = coordToSvg(item.extent.p2);
      const x = Math.min(p1.x, p2.x);
      const y = Math.min(p1.y, p2.y);
      const w = Math.abs(p2.x - p1.x);
      const h = Math.abs(p2.y - p1.y);
      const inner = `<image href="${escapeXmlAttr(item.fileName)}" x="${x}" y="${y}" width="${w}" height="${h}"/>`;
      return wrapItemSvg(inner, item, extent, svgWidth, svgHeight);
    }

    case "Group": {
      const inner = item.children
        .map((c, j) => renderGraphicItemToSvg(c, `${pathId}-${j}`, extent, svgWidth, svgHeight))
        .join("\n");
      return wrapItemSvg(inner, item, extent, svgWidth, svgHeight);
    }

    default:
      return "";
  }
}
