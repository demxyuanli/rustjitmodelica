import type { DiagramDocument } from "../api/types";

/** Download a minimal SVG snapshot of the current diagram (nodes as labeled rects). */
export function exportDiagramSvg(diagram: DiagramDocument, fileName: string) {
  const nodes = diagram.nodes.filter((n) => n.nodeKind !== "draw" || (n.ui?.width ?? 0) > 0);
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const n of nodes) {
    const w = n.ui?.width ?? 72;
    const h = n.ui?.height ?? 40;
    minX = Math.min(minX, n.position.x);
    minY = Math.min(minY, n.position.y);
    maxX = Math.max(maxX, n.position.x + w);
    maxY = Math.max(maxY, n.position.y + h);
  }
  if (!Number.isFinite(minX)) {
    minX = 0;
    minY = 0;
    maxX = 200;
    maxY = 120;
  }
  const pad = 24;
  const W = Math.max(120, maxX - minX + pad * 2);
  const H = Math.max(80, maxY - minY + pad * 2);
  const parts: string[] = [
    `<?xml version="1.0" encoding="UTF-8"?>`,
    `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">`,
    `<rect width="100%" height="100%" fill="#1e1e1e"/>`,
  ];
  for (const e of diagram.edges) {
    const src = nodes.find((n) => n.id === e.sourceNodeId);
    const tgt = nodes.find((n) => n.id === e.targetNodeId);
    if (!src || !tgt) continue;
    const sw = src.ui?.width ?? 72;
    const sh = src.ui?.height ?? 40;
    const th = tgt.ui?.height ?? 40;
    const x1 = src.position.x - minX + pad + sw;
    const y1 = src.position.y - minY + pad + sh / 2;
    const x2 = tgt.position.x - minX + pad;
    const y2 = tgt.position.y - minY + pad + th / 2;
    parts.push(
      `<path d="M${x1} ${y1} L${(x1 + x2) / 2} ${y1} L${(x1 + x2) / 2} ${y2} L${x2} ${y2}" fill="none" stroke="#7aa2f7" stroke-width="1.5"/>`,
    );
  }
  for (const n of nodes) {
    const w = n.ui?.width ?? 72;
    const h = n.ui?.height ?? 40;
    const x = n.position.x - minX + pad;
    const y = n.position.y - minY + pad;
    const label = (n.ui?.label ?? n.instanceName).replace(/[<>&"]/g, "");
    parts.push(
      `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="2" fill="#2d2d2d" stroke="#888" stroke-width="1.2"/>`,
      `<text x="${x + 6}" y="${y + h / 2 + 4}" font-size="11" fill="#ddd" font-family="Segoe UI,sans-serif">${label}</text>`,
    );
  }
  parts.push(`</svg>`);
  const blob = new Blob([parts.join("\n")], { type: "image/svg+xml;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = fileName.endsWith(".svg") ? fileName : `${fileName}.svg`;
  a.click();
  URL.revokeObjectURL(url);
}
