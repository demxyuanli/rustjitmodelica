import type {
  CatalogEntry,
  DiagramDocument,
  DiagramEdge,
  DiagramNode,
  ParamValue,
  PortDef,
  Position,
} from "../api/types";

const DEFAULT_HANDLE = "__default__";
const GRID_X = 180;
const GRID_Y = 100;

export type ImportIdeResult =
  | {
      ok: true;
      diagram: DiagramDocument;
      unmatchedTypes: string[];
      skippedEdges: number;
      alreadyStudio: boolean;
    }
  | { ok: false; error: string };

type IdePoint = { x?: unknown; y?: unknown };

type IdeExtent = { p1?: IdePoint; p2?: IdePoint };

type IdeComponent = {
  name?: unknown;
  typeName?: unknown;
  rotation?: unknown;
  origin?: IdePoint;
  params?: unknown;
  icon?: unknown;
  isInput?: unknown;
  isOutput?: unknown;
  connectorKind?: unknown;
  placement?: {
    transformation?: {
      origin?: IdePoint;
      extent?: IdeExtent;
      rotation?: unknown;
    };
  };
};

type IdeConnection = {
  from?: unknown;
  to?: unknown;
  line?: {
    points?: IdePoint[];
    routing?: unknown;
  };
};

type IdeModel = {
  modelName: string;
  components: IdeComponent[];
  connections: IdeConnection[];
  layout?: Record<string, IdePoint>;
};

function asNum(v: unknown, fallback = 0): number {
  return typeof v === "number" && Number.isFinite(v) ? v : fallback;
}

function asStr(v: unknown, fallback = ""): string {
  return typeof v === "string" ? v : fallback;
}

function lastSeg(s: string): string {
  const i = s.lastIndexOf(".");
  return (i >= 0 ? s.slice(i + 1) : s).toLowerCase();
}

function pathToNodeAndHandle(path: string): { nodeId: string; handleId: string } {
  const dot = path.indexOf(".");
  if (dot < 0) return { nodeId: path, handleId: DEFAULT_HANDLE };
  return { nodeId: path.slice(0, dot), handleId: path.slice(dot + 1) };
}

function guessDomain(typeName: string): string {
  const n = typeName.toLowerCase();
  if (n.includes("electrical") || n.includes("analog")) return "electrical";
  if (n.includes("multibody")) return "multibody";
  if (n.includes("mechanics")) return "mechanical";
  return "signal";
}

function guessPortKind(id: string): string {
  const n = id.toLowerCase();
  if (
    n === "p" ||
    n === "n" ||
    n.startsWith("pin") ||
    n.startsWith("flange") ||
    n.startsWith("frame")
  ) {
    return "connector";
  }
  return "signal";
}

function catalogSize(entry: CatalogEntry): { width: number; height: number } {
  const tags = entry.tags ?? [];
  if (tags.includes("terminal")) return { width: 72, height: 36 };
  if (tags.includes("jump")) return { width: 64, height: 40 };
  if (entry.icon) return { width: 148, height: 80 };
  return { width: 120, height: 48 };
}

function extentSize(ext: IdeExtent | undefined): { width: number; height: number } | null {
  if (!ext?.p1 || !ext?.p2) return null;
  const w = Math.abs(asNum(ext.p2.x) - asNum(ext.p1.x));
  const h = Math.abs(asNum(ext.p2.y) - asNum(ext.p1.y));
  if (w < 40 || h < 24) return null;
  return { width: w, height: h };
}

export function isStudioDiagramDocument(raw: unknown): raw is DiagramDocument {
  if (!raw || typeof raw !== "object") return false;
  const o = raw as Record<string, unknown>;
  return (
    typeof o.modelName === "string" &&
    Array.isArray(o.nodes) &&
    Array.isArray(o.edges)
  );
}

function normalizeIdeModel(raw: unknown): IdeModel | null {
  if (!raw || typeof raw !== "object") return null;
  const o = raw as Record<string, unknown>;
  const graphical =
    o.graphical && typeof o.graphical === "object"
      ? (o.graphical as Record<string, unknown>)
      : null;
  if (!Array.isArray(o.components) || !Array.isArray(o.connections)) return null;
  const modelName = asStr(o.modelName, "Imported");
  const layoutRaw = (graphical?.layout ?? o.layout) as unknown;
  const layout =
    layoutRaw && typeof layoutRaw === "object" && !Array.isArray(layoutRaw)
      ? (layoutRaw as Record<string, IdePoint>)
      : undefined;
  return {
    modelName: modelName || "Imported",
    components: o.components as IdeComponent[],
    connections: o.connections as IdeConnection[],
    layout,
  };
}

export function matchCatalog(
  typeName: string,
  catalog: CatalogEntry[],
): CatalogEntry | undefined {
  const needle = typeName.toLowerCase();
  if (!needle) return undefined;
  const exact = catalog.find((c) => c.typeName.toLowerCase() === needle);
  if (exact) return exact;
  const scored: { c: CatalogEntry; score: number }[] = [];
  for (const c of catalog) {
    const tn = c.typeName.toLowerCase();
    if (tn.endsWith(`.${needle}`) || needle.endsWith(`.${tn}`)) {
      scored.push({ c, score: 500 + Math.min(tn.length, needle.length) });
    } else if (lastSeg(tn) === lastSeg(needle)) {
      scored.push({ c, score: 100 });
    }
  }
  scored.sort((a, b) => b.score - a.score);
  if (scored.length === 0) return undefined;
  const top = scored[0]!;
  const second = scored[1];
  if (second && second.score === top.score && top.score < 500) return undefined;
  return top.c;
}

function componentPosition(
  comp: IdeComponent,
  name: string,
  layout: Record<string, IdePoint> | undefined,
  index: number,
): Position {
  const laid = layout?.[name];
  if (laid && typeof laid === "object") {
    return { x: asNum(laid.x), y: asNum(laid.y) };
  }
  const xf = comp.placement?.transformation;
  const ext = xf?.extent;
  const origin = xf?.origin ?? comp.origin;
  if (ext?.p1 && ext?.p2) {
    const ox = asNum(origin?.x);
    const oy = asNum(origin?.y);
    return {
      x: (asNum(ext.p1.x) + asNum(ext.p2.x)) / 2 + ox + 200,
      y: 200 - ((asNum(ext.p1.y) + asNum(ext.p2.y)) / 2 + oy),
    };
  }
  return { x: (index % 4) * GRID_X, y: Math.floor(index / 4) * GRID_Y };
}

function parseParams(raw: unknown): ParamValue[] {
  if (!Array.isArray(raw)) return [];
  const out: ParamValue[] = [];
  for (const item of raw) {
    if (!item || typeof item !== "object") continue;
    const p = item as Record<string, unknown>;
    const name = asStr(p.name);
    if (!name) continue;
    out.push({ name, value: asStr(p.value) });
  }
  return out;
}

function synthPort(id: string, direction: string): PortDef {
  return {
    id,
    kind: guessPortKind(id),
    direction,
    typeName: direction === "in" ? "RealInput" : direction === "out" ? "RealOutput" : "",
  };
}

function resolveHandle(
  node: DiagramNode,
  handleId: string,
  role: "source" | "target",
): string {
  if (handleId && handleId !== DEFAULT_HANDLE) {
    const hit = node.ports.find((p) => p.id === handleId);
    if (hit) return hit.id;
    const ci = node.ports.find((p) => p.id.toLowerCase() === handleId.toLowerCase());
    if (ci) return ci.id;
    const dir = role === "source" ? "out" : "in";
    node.ports.push(synthPort(handleId, dir));
    return handleId;
  }
  const prefer = role === "source" ? "out" : "in";
  const byDir = node.ports.find((p) => p.direction === prefer);
  if (byDir) return byDir.id;
  if (node.ports[0]) return node.ports[0].id;
  const fallback = role === "source" ? "y" : "u";
  node.ports.push(synthPort(fallback, prefer));
  return fallback;
}

function toVertices(points: IdePoint[] | undefined): Position[] {
  if (!points?.length) return [];
  return points.map((p) => ({ x: asNum(p.x), y: asNum(p.y) }));
}

export function importIdeDiagram(raw: unknown, catalog: CatalogEntry[]): ImportIdeResult {
  if (isStudioDiagramDocument(raw)) {
    const modelName = raw.modelName.trim() || "Imported";
    return {
      ok: true,
      alreadyStudio: true,
      unmatchedTypes: [],
      skippedEdges: 0,
      diagram: {
        schemaVersion: typeof raw.schemaVersion === "number" ? raw.schemaVersion : 1,
        modelName,
        diagramId: raw.diagramId || "root",
        parentDiagramId: raw.parentDiagramId ?? null,
        nodes: raw.nodes ?? [],
        edges: raw.edges ?? [],
        annotations: raw.annotations ?? [],
        viewport: raw.viewport ?? { x: 0, y: 0, zoom: 1 },
      },
    };
  }

  const ide = normalizeIdeModel(raw);
  if (!ide) {
    return {
      ok: false,
      error: "Not an IDE DiagramModel (need components/connections) or Studio DiagramDocument.",
    };
  }

  const unmatchedTypes: string[] = [];
  const nodes: DiagramNode[] = [];
  const byName = new Map<string, DiagramNode>();

  ide.components.forEach((comp, index) => {
    const name = asStr(comp.name).trim();
    if (!name) return;
    const typeName = asStr(comp.typeName, "Block") || "Block";
    const entry = matchCatalog(typeName, catalog);
    if (!entry) unmatchedTypes.push(typeName);
    const domain = entry?.domain ?? guessDomain(typeName);
    const size = entry ? catalogSize(entry) : { width: 120, height: 48 };
    const fromExtent = extentSize(comp.placement?.transformation?.extent);
    const rotation = asNum(comp.rotation, asNum(comp.placement?.transformation?.rotation));
    const params = parseParams(comp.params);
    const mergedParams =
      entry && params.length === 0
        ? entry.defaultParams.map((p) => ({ name: p.name, value: p.value }))
        : params;
    const ports = (entry?.ports ?? []).map((p) => ({ ...p }));
    if (ports.length === 0) {
      if (comp.isInput) ports.push(synthPort("u", "in"));
      if (comp.isOutput) ports.push(synthPort("y", "out"));
      if (ports.length === 0) {
        ports.push(synthPort("u", "in"));
        ports.push(synthPort("y", "out"));
      }
    }
    const node: DiagramNode = {
      id: `n_${name}`,
      instanceName: name,
      typeName: entry?.typeName ?? typeName,
      domain,
      catalogId: entry?.id ?? `imported.${lastSeg(typeName) || "block"}`,
      position: componentPosition(comp, name, ide.layout, index),
      rotation,
      params: mergedParams,
      ports,
      nodeKind: "component",
      subsystemRef: null,
      plotHints: [...(entry?.plotHints ?? [])],
      ui: {
        label: name,
        width: fromExtent?.width ?? size.width,
        height: fromExtent?.height ?? size.height,
        icon: (comp.icon ?? entry?.icon ?? undefined) as unknown,
      },
    };
    nodes.push(node);
    byName.set(name, node);
  });

  const edges: DiagramEdge[] = [];
  let skippedEdges = 0;
  ide.connections.forEach((conn, i) => {
    const from = asStr(conn.from);
    const to = asStr(conn.to);
    if (!from || !to) {
      skippedEdges += 1;
      return;
    }
    const a = pathToNodeAndHandle(from);
    const b = pathToNodeAndHandle(to);
    const src = byName.get(a.nodeId);
    const dst = byName.get(b.nodeId);
    if (!src || !dst) {
      skippedEdges += 1;
      return;
    }
    const sourcePortId = resolveHandle(src, a.handleId, "source");
    const targetPortId = resolveHandle(dst, b.handleId, "target");
    edges.push({
      id: `e_${i}_${src.id}_${sourcePortId}_${dst.id}_${targetPortId}`,
      sourceNodeId: src.id,
      sourcePortId,
      targetNodeId: dst.id,
      targetPortId,
      vertices: toVertices(conn.line?.points),
      routing: asStr(conn.line?.routing) || undefined,
    });
  });

  return {
    ok: true,
    alreadyStudio: false,
    unmatchedTypes: [...new Set(unmatchedTypes)],
    skippedEdges,
    diagram: {
      schemaVersion: 1,
      modelName: ide.modelName,
      diagramId: "root",
      parentDiagramId: null,
      nodes,
      edges,
      annotations: [],
      viewport: { x: 0, y: 0, zoom: 1 },
    },
  };
}
