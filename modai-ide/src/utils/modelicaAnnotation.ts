export interface AnnotationTagEntry {
  name: string;
  value: string;
}

export interface IconShape {
  type: "Rectangle" | "Ellipse" | "Line" | "Polygon" | "Unknown";
  extent?: [number, number, number, number];
  points?: Array<[number, number]>;
  lineColorRGB?: [number, number, number];
  fillColorRGB?: [number, number, number];
}

export interface IconGraphics {
  extent: [number, number, number, number];
  shapes: IconShape[];
  hasTextLabel?: boolean;
}

export interface ExperimentInfo {
  startTime?: number;
  stopTime?: number;
  interval?: number;
  tolerance?: number;
}

export interface VersionInfo {
  version?: string;
  versionDate?: string;
  versionBuild?: string;
}

export interface UsesInfoEntry {
  library: string;
  version?: string;
}

export interface AnnotationViewModel {
  documentationInfo?: string;
  documentationRevisions?: string;
  experiment?: ExperimentInfo;
  version?: VersionInfo;
  uses?: UsesInfoEntry[];
  iconGraphics?: IconGraphics | null;
  rawEntries: AnnotationTagEntry[];
}

function parseNumber(value: string | undefined): number | undefined {
  if (!value) return undefined;
  const n = Number(value);
  return Number.isFinite(n) ? n : undefined;
}

function extractTopLevelAnnotationInner(code: string): string | null {
  const match = code.match(/\bannotation\s*\(/);
  if (!match || match.index === undefined) return null;
  const open = match.index + match[0].length - 1;
  let depth = 1;
  for (let i = open + 1; i < code.length; i++) {
    const c = code[i];
    if (c === "(") depth++;
    else if (c === ")") {
      depth--;
      if (depth === 0) {
        return code.slice(open + 1, i);
      }
    }
  }
  return null;
}

function parsePoints(source: string): Array<[number, number]> {
  const out: Array<[number, number]> = [];
  const re = /\{\s*([-\d.]+)\s*,\s*([-\d.]+)\s*\}/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(source)) !== null) {
    out.push([parseFloat(m[1]), parseFloat(m[2])]);
  }
  return out;
}

function extractIconBlock(inner: string): string | null {
  const idx = inner.indexOf("Icon(");
  if (idx < 0) return null;
  const start = idx + 5;
  let depth = 1;
  for (let i = start; i < inner.length; i++) {
    const c = inner[i];
    if (c === "(") depth++;
    else if (c === ")") {
      depth--;
      if (depth === 0) {
        return inner.slice(start, i);
      }
    }
  }
  return null;
}

export function extractIconGraphics(inner: string): IconGraphics | null {
  const iconInner = extractIconBlock(inner);
  if (!iconInner) return null;
  const graphicsIdx = iconInner.indexOf("graphics={");
  if (graphicsIdx < 0) return null;
  const graphicsStart = graphicsIdx + "graphics=".length + 1;
  let depth = 1;
  let end = graphicsStart;
  for (let i = graphicsStart; i < iconInner.length; i++) {
    const c = iconInner[i];
    if (c === "{") depth++;
    else if (c === "}") {
      depth--;
      if (depth === 0) {
        end = i;
        break;
      }
    }
  }
  const graphicsStr = iconInner.slice(graphicsStart, end);
  let extent: [number, number, number, number] = [-100, -100, 100, 100];
  const extentMatch = iconInner.match(
    /extent\s*=\s*\{\{\s*([-\d.]+)\s*,\s*([-\d.]+)\s*\},\s*\{\s*([-\d.]+)\s*,\s*([-\d.]+)\s*\}\}/
  );
  if (extentMatch) {
    extent = [
      parseFloat(extentMatch[1]),
      parseFloat(extentMatch[2]),
      parseFloat(extentMatch[3]),
      parseFloat(extentMatch[4]),
    ];
  }
  const shapes: IconShape[] = [];
  let m: RegExpExecArray | null;

  // Rectangles
  const rectRe = /Rectangle\s*\(([^)]*)\)/g;
  while ((m = rectRe.exec(graphicsStr)) !== null) {
    const body = m[1];
    const ext = body.match(
      /extent\s*=\s*\{\{\s*([-\d.]+)\s*,\s*([-\d.]+)\s*\},\s*\{\s*([-\d.]+)\s*,\s*([-\d.]+)\s*\}\}/
    );
    const line = body.match(/lineColor\s*=\s*\{\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\}/);
    const fill = body.match(/fillColor\s*=\s*\{\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\}/);
    const shape: IconShape = { type: "Rectangle" };
    if (ext) {
      shape.extent = [
        parseFloat(ext[1]),
        parseFloat(ext[2]),
        parseFloat(ext[3]),
        parseFloat(ext[4]),
      ];
    }
    if (line) {
      shape.lineColorRGB = [Number(line[1]), Number(line[2]), Number(line[3])];
    }
    if (fill) {
      shape.fillColorRGB = [Number(fill[1]), Number(fill[2]), Number(fill[3])];
    }
    shapes.push(shape);
  }

  // Ellipses
  const ellipseRe = /Ellipse\s*\(([^)]*)\)/g;
  while ((m = ellipseRe.exec(graphicsStr)) !== null) {
    const body = m[1];
    const ext = body.match(
      /extent\s*=\s*\{\{\s*([-\d.]+)\s*,\s*([-\d.]+)\s*\},\s*\{\s*([-\d.]+)\s*,\s*([-\d.]+)\s*\}\}/
    );
    const line = body.match(/lineColor\s*=\s*\{\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\}/);
    const fill = body.match(/fillColor\s*=\s*\{\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\}/);
    const shape: IconShape = { type: "Ellipse" };
    if (ext) {
      shape.extent = [
        parseFloat(ext[1]),
        parseFloat(ext[2]),
        parseFloat(ext[3]),
        parseFloat(ext[4]),
      ];
    }
    if (line) {
      shape.lineColorRGB = [Number(line[1]), Number(line[2]), Number(line[3])];
    }
    if (fill) {
      shape.fillColorRGB = [Number(fill[1]), Number(fill[2]), Number(fill[3])];
    }
    shapes.push(shape);
  }

  // Lines
  const lineRe = /Line\s*\(([^)]*)\)/g;
  while ((m = lineRe.exec(graphicsStr)) !== null) {
    const body = m[1];
    const ptsMatch = body.match(/points\s*=\s*\{([\s\S]*)\}/);
    if (!ptsMatch) continue;
    const pts = parsePoints(ptsMatch[1]);
    if (pts.length >= 2) {
      const line = body.match(/lineColor\s*=\s*\{\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\}/);
      const shape: IconShape = { type: "Line", points: pts };
      if (line) {
        shape.lineColorRGB = [Number(line[1]), Number(line[2]), Number(line[3])];
      }
      shapes.push(shape);
    }
  }

  // Polygons
  const polyRe = /Polygon\s*\(([^)]*)\)/g;
  while ((m = polyRe.exec(graphicsStr)) !== null) {
    const body = m[1];
    const ptsMatch = body.match(/points\s*=\s*\{([\s\S]*)\}/);
    if (!ptsMatch) continue;
    const pts = parsePoints(ptsMatch[1]);
    if (pts.length >= 2) {
      const line = body.match(/lineColor\s*=\s*\{\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\}/);
      const fill = body.match(/fillColor\s*=\s*\{\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\}/);
      const shape: IconShape = { type: "Polygon", points: pts };
      if (line) {
        shape.lineColorRGB = [Number(line[1]), Number(line[2]), Number(line[3])];
      }
      if (fill) {
        shape.fillColorRGB = [Number(fill[1]), Number(fill[2]), Number(fill[3])];
      }
      shapes.push(shape);
    }
  }

  const hasTextLabel = /\bText\s*\([^)]*textString\s*=\s*"%name"/.test(graphicsStr);

  if (shapes.length === 0) return null;
  return { extent, shapes, hasTextLabel };
}

export function parseAnnotationViewModel(code: string): AnnotationViewModel {
  const inner = extractTopLevelAnnotationInner(code);
  if (!inner) {
    return { rawEntries: [] };
  }

  const rawEntries: AnnotationTagEntry[] = [];
  let documentationInfo: string | undefined;
  let documentationRevisions: string | undefined;
  let experiment: ExperimentInfo | undefined;
  let version: VersionInfo | undefined;
  const uses: UsesInfoEntry[] = [];

  const docInfoDouble = inner.match(/Documentation\s*\(\s*info\s*=\s*"((?:[^"\\]|\\.)*)"/);
  if (docInfoDouble) {
    documentationInfo = docInfoDouble[1].replace(/\\"/g, '"');
    rawEntries.push({ name: "Documentation.info", value: documentationInfo });
  }
  const docInfoTriple = inner.match(/Documentation\s*\(\s*info\s*=\s*"""([\s\S]*?)"""/);
  if (docInfoTriple) {
    documentationInfo = docInfoTriple[1].trim();
    rawEntries.push({ name: "Documentation.info", value: documentationInfo });
  }
  const docRevisions = inner.match(/Documentation\s*\([^)]*revisions\s*=\s*"""([\s\S]*?)"""/);
  if (docRevisions) {
    documentationRevisions = docRevisions[1].trim();
    rawEntries.push({ name: "Documentation.revisions", value: documentationRevisions });
  }

  const experimentBlock = inner.match(/experiment\s*\(([^)]*)\)/);
  if (experimentBlock) {
    const body = experimentBlock[1];
    const get = (key: string) => {
      const m = body.match(new RegExp(`${key}\\s*=\\s*([\\d.eE+-]+)`));
      return m ? m[1] : undefined;
    };
    experiment = {
      startTime: parseNumber(get("StartTime")),
      stopTime: parseNumber(get("StopTime")),
      interval: parseNumber(get("Interval")),
      tolerance: parseNumber(get("Tolerance")),
    };
    rawEntries.push({
      name: "experiment",
      value: body.trim(),
    });
  }

  const versionMatch = inner.match(/version\s*=\s*"([^"]*)"/);
  const versionDateMatch = inner.match(/versionDate\s*=\s*"([^"]*)"/);
  const versionBuildMatch = inner.match(/versionBuild\s*=\s*"([^"]*)"/);
  if (versionMatch || versionDateMatch || versionBuildMatch) {
    version = {
      version: versionMatch?.[1],
      versionDate: versionDateMatch?.[1],
      versionBuild: versionBuildMatch?.[1],
    };
    if (version.version) rawEntries.push({ name: "version", value: version.version });
    if (version.versionDate) rawEntries.push({ name: "versionDate", value: version.versionDate });
    if (version.versionBuild) rawEntries.push({ name: "versionBuild", value: version.versionBuild });
  }

  const usesRe = /uses\s*\(\s*([^)]+)\)/g;
  let um: RegExpExecArray | null;
  while ((um = usesRe.exec(inner)) !== null) {
    const body = um[1];
    const parts = body.split(",").map((s) => s.trim());
    const lib = parts[0];
    let ver: string | undefined;
    const verMatch = body.match(/version\s*=\s*"([^"]*)"/);
    if (verMatch) ver = verMatch[1];
    if (lib) {
      uses.push({ library: lib, version: ver });
      rawEntries.push({
        name: "uses",
        value: ver ? `${lib} (version=${ver})` : lib,
      });
    }
  }

  if (rawEntries.length === 0 && inner.trim()) {
    rawEntries.push({
      name: "annotation",
      value: inner.trim().slice(0, 200) + (inner.length > 200 ? "..." : ""),
    });
  }

  return {
    documentationInfo,
    documentationRevisions,
    experiment,
    version,
    uses: uses.length ? uses : undefined,
    iconGraphics: extractIconGraphics(inner),
    rawEntries,
  };
}

export function parseAnnotationViewModelForType(code: string, qualifiedName: string): AnnotationViewModel {
  const simpleName = qualifiedName.split(".").pop() ?? qualifiedName;
  const endRe = new RegExp(`\\bend\\s+${simpleName}\\s*;`);
  const endMatch = endRe.exec(code);
  if (!endMatch) {
    return parseAnnotationViewModel(code);
  }
  const endIndex = endMatch.index + endMatch[0].length;
  const headerRe = new RegExp(
    `(^|\\n)\\s*(partial\\s+)?(model|block|connector|record|package|class|function)\\s+${simpleName}\\b`,
    "g"
  );
  let headerIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = headerRe.exec(code)) !== null) {
    if (m.index < endMatch.index) {
      headerIndex = m.index;
    } else {
      break;
    }
  }
  const snippet = code.slice(headerIndex, endIndex);
  return parseAnnotationViewModel(snippet);
}


