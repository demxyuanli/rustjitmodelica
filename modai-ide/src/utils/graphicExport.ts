/**
 * Graphic export utilities
 * Provides functions to export graphics as SVG and PNG
 */

import type { GraphicItem, IconDiagramAnnotation, AnnotationPoint, AnnotationColor } from "../components/DiagramSvgRenderer";

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

  // Generate SVG content
  const graphicsSvg = annotation.graphics
    .map((item, idx) => renderGraphicItemToSvg(item, idx, extent, width, height))
    .join("\n");

  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" 
     width="${width}" 
     height="${height}" 
     viewBox="0 0 ${width} ${height}">
  <rect width="100%" height="100%" fill="${backgroundColor}"/>
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
  _idx: number,
  extent: { p1: AnnotationPoint; p2: AnnotationPoint },
  svgWidth: number,
  svgHeight: number,
): string {
  const cw = Math.abs(extent.p2.x - extent.p1.x) || 200;
  const ch = Math.abs(extent.p2.y - extent.p1.y) || 200;
  const minX = Math.min(extent.p1.x, extent.p2.x);
  const maxY = Math.max(extent.p1.y, extent.p2.y);

  const coordToSvg = (p: AnnotationPoint) => ({
    x: ((p.x - minX) / cw) * svgWidth,
    y: ((maxY - p.y) / ch) * svgHeight,
  });

  const colorToSvg = (c?: { r: number; g: number; b: number }) => {
    if (!c) return "currentColor";
    return `rgb(${c.r},${c.g},${c.b})`;
  };

  const isFilled = (pattern?: string) => {
    if (!pattern) return false;
    const lower = pattern.toLowerCase();
    return lower !== "none" && (
      lower.includes("solid") ||
      lower.includes("horizontal") ||
      lower.includes("vertical") ||
      lower.includes("cross")
    );
  };

  switch (item.type) {
    case "Line":
    case "BSpline": {
      if (item.points.length < 2) return "";
      const pts = item.points.map(coordToSvg);
      const d = pts.map((p, i) => `${i === 0 ? "M" : "L"} ${p.x} ${p.y}`).join(" ");
      const colorObj: AnnotationColor | undefined = "color" in item ? item.color : ("lineColor" in item ? item.lineColor as AnnotationColor | undefined : undefined);
      const color = colorToSvg(colorObj);
      const thickness = "thickness" in item ? item.thickness : ("lineThickness" in item ? (item.lineThickness as number | undefined) : 1) ?? 1;
      return `<path d="${d}" stroke="${color}" stroke-width="${thickness}" fill="none"/>`;
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
      const fill = isFilled(item.fillPattern) ? colorToSvg(item.fillColor) : "none";
      const strokeWidth = item.lineThickness ?? 1;
      const rx = item.radius ?? 0;
      return `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="${rx}" stroke="${stroke}" stroke-width="${strokeWidth}" fill="${fill}"/>`;
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
      const fill = isFilled(item.fillPattern) ? colorToSvg(item.fillColor) : "none";
      const strokeWidth = item.lineThickness ?? 1;
      return `<ellipse cx="${cx}" cy="${cy}" rx="${rx}" ry="${ry}" stroke="${stroke}" stroke-width="${strokeWidth}" fill="${fill}"/>`;
    }

    case "Polygon": {
      if (item.points.length < 2) return "";
      const pts = item.points.map(coordToSvg).map(p => `${p.x},${p.y}`).join(" ");
      const stroke = colorToSvg(item.lineColor);
      const fill = isFilled(item.fillPattern) ? colorToSvg(item.fillColor) : "none";
      const strokeWidth = item.lineThickness ?? 1;
      return `<polygon points="${pts}" stroke="${stroke}" stroke-width="${strokeWidth}" fill="${fill}"/>`;
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
      return `<text x="${x}" y="${y}" text-anchor="middle" dominant-baseline="central" font-size="${fontSize}" fill="${fill}" font-family="${fontFamily}">${text}</text>`;
    }

    case "Bitmap": {
      if (!item.extent || !item.fileName) return "";
      const p1 = coordToSvg(item.extent.p1);
      const p2 = coordToSvg(item.extent.p2);
      const x = Math.min(p1.x, p2.x);
      const y = Math.min(p1.y, p2.y);
      const w = Math.abs(p2.x - p1.x);
      const h = Math.abs(p2.y - p1.y);
      return `<image href="${item.fileName}" x="${x}" y="${y}" width="${w}" height="${h}"/>`;
    }

    default:
      return "";
  }
}
