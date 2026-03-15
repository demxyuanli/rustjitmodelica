import { dia, shapes } from "@joint/core";
import { getConnectorColor, resolveDiagramColors } from "../../utils/jointUtils";

const PORT_SIZE = 6;

function portMarkup() {
  return [
    {
      tagName: "rect",
      selector: "portBody",
      attributes: {
        width: PORT_SIZE,
        height: PORT_SIZE,
        x: -PORT_SIZE / 2,
        y: -PORT_SIZE / 2,
        fill: "#888",
        stroke: "none",
      },
    },
  ];
}

export type NodeShape = "block" | "source" | "sink";

export function determineNodeShape(opts: {
  isInput?: boolean;
  isOutput?: boolean;
  isSourceNode?: boolean;
  isSinkNode?: boolean;
}): NodeShape {
  const isInputOnly = Boolean(opts.isInput && !opts.isOutput);
  const isOutputOnly = Boolean(opts.isOutput && !opts.isInput);
  if (isInputOnly || opts.isSourceNode) return "source";
  if (isOutputOnly || opts.isSinkNode) return "sink";
  return "block";
}

export interface CreateElementOptions {
  id: string;
  shape: NodeShape;
  position: { x: number; y: number };
  label: string;
  sublabel?: string;
  paramStr?: string;
  ports: string[];
  connectorKind?: string;
  hasError?: boolean;
  errorMessage?: string;
}

function createBlockElement(opts: CreateElementOptions, colors: ReturnType<typeof resolveDiagramColors>, connectorColor: string): dia.Element {
  const fullLabel = opts.sublabel ? `${opts.label}\n${opts.sublabel}` : opts.label;
  const displayLabel = opts.paramStr ? `${fullLabel}\n${opts.paramStr}` : fullLabel;
  const lines = displayLabel.split("\n");
  const height = Math.max(60, 20 + lines.length * 14);
  const maxLineLen = Math.max(...lines.map((l) => l.length));
  const width = Math.max(160, maxLineLen * 7 + 20);

  const el = new shapes.standard.Rectangle({
    id: opts.id,
    position: opts.position,
    size: { width, height },
    attrs: {
      body: {
        fill: colors.bgElevated,
        stroke: opts.hasError ? "#ef4444" : colors.border,
        strokeWidth: 1,
        rx: 4,
        ry: 4,
      },
      label: {
        text: displayLabel,
        fill: colors.text,
        fontSize: 11,
        fontWeight: 600,
        textVerticalAnchor: "top",
        textAnchor: "start",
        refX: 8,
        refY: 8,
      },
    },
    ports: {
      groups: {
        input: {
          position: { name: "left" },
          attrs: { portBody: { magnet: "passive", width: PORT_SIZE, height: PORT_SIZE, x: -PORT_SIZE / 2, y: -PORT_SIZE / 2 } },
          markup: portMarkup(),
        },
        output: {
          position: { name: "right" },
          attrs: { portBody: { magnet: true, width: PORT_SIZE, height: PORT_SIZE, x: -PORT_SIZE / 2, y: -PORT_SIZE / 2 } },
          markup: portMarkup(),
        },
      },
    },
  });

  opts.ports.forEach((port) => {
    el.addPort({ id: `in_${port}`, group: "input", attrs: { portBody: { fill: connectorColor } } });
    el.addPort({ id: `out_${port}`, group: "output", attrs: { portBody: { fill: connectorColor } } });
  });

  return el;
}

function createSourceElement(opts: CreateElementOptions, colors: ReturnType<typeof resolveDiagramColors>, connectorColor: string): dia.Element {
  const el = new shapes.standard.Path({
    id: opts.id,
    position: opts.position,
    size: { width: 48, height: 48 },
    attrs: {
      body: {
        d: "M 0 0 L 48 24 L 0 48 Z",
        fill: colors.bgElevated,
        stroke: opts.hasError ? "#ef4444" : colors.border,
        strokeWidth: 1,
      },
      label: {
        text: opts.label,
        fill: colors.text,
        fontSize: 9,
        fontWeight: 600,
        textVerticalAnchor: "middle",
        textAnchor: "middle",
        refX: "35%",
        refY: "50%",
      },
    },
    ports: {
      groups: {
        output: {
          position: { name: "right" },
          attrs: { portBody: { magnet: true, width: PORT_SIZE, height: PORT_SIZE, x: -PORT_SIZE / 2, y: -PORT_SIZE / 2 } },
          markup: portMarkup(),
        },
      },
    },
  });

  opts.ports.forEach((port) => {
    el.addPort({ id: port, group: "output", attrs: { portBody: { fill: connectorColor } } });
  });

  return el;
}

function createSinkElement(opts: CreateElementOptions, colors: ReturnType<typeof resolveDiagramColors>, connectorColor: string): dia.Element {
  const el = new shapes.standard.Circle({
    id: opts.id,
    position: opts.position,
    size: { width: 48, height: 48 },
    attrs: {
      body: {
        fill: colors.bgElevated,
        stroke: opts.hasError ? "#ef4444" : colors.border,
        strokeWidth: 1,
      },
      label: {
        text: opts.label,
        fill: colors.text,
        fontSize: 9,
        fontWeight: 600,
        textVerticalAnchor: "middle",
        textAnchor: "middle",
      },
    },
    ports: {
      groups: {
        input: {
          position: { name: "left" },
          attrs: { portBody: { magnet: "passive", width: PORT_SIZE, height: PORT_SIZE, x: -PORT_SIZE / 2, y: -PORT_SIZE / 2 } },
          markup: portMarkup(),
        },
      },
    },
  });

  opts.ports.forEach((port) => {
    el.addPort({ id: port, group: "input", attrs: { portBody: { fill: connectorColor } } });
  });

  return el;
}

export function createElement(opts: CreateElementOptions): dia.Element {
  const colors = resolveDiagramColors();
  const connectorColor = getConnectorColor(opts.connectorKind);

  let el: dia.Element;
  switch (opts.shape) {
    case "source":
      el = createSourceElement(opts, colors, connectorColor);
      break;
    case "sink":
      el = createSinkElement(opts, colors, connectorColor);
      break;
    default:
      el = createBlockElement(opts, colors, connectorColor);
      break;
  }

  if (opts.hasError) {
    el.set("hasError", true);
    el.set("errorMessage", opts.errorMessage);
  }

  return el;
}

export function resolvePortId(
  graph: dia.Graph,
  elementId: string,
  portId: string,
  side: "source" | "target"
): string {
  const el = graph.getCell(elementId);
  if (!el || !el.isElement()) return portId;
  const element = el as dia.Element;
  const ports = element.getPorts();
  if (ports.find((p) => p.id === portId)) return portId;
  const prefixed = side === "source" ? `out_${portId}` : `in_${portId}`;
  if (ports.find((p) => p.id === prefixed)) return prefixed;
  const firstPort = ports.find((p) =>
    side === "source" ? p.group === "output" : p.group === "input"
  );
  if (firstPort) return firstPort.id!;
  return portId;
}

export function createLink(
  opts: {
    id: string;
    source: string;
    sourcePort: string;
    target: string;
    targetPort: string;
    vertices?: { x: number; y: number }[];
  },
  graph?: dia.Graph
): dia.Link {
  const colors = resolveDiagramColors();

  let sourcePort = opts.sourcePort;
  let targetPort = opts.targetPort;

  if (graph) {
    sourcePort = resolvePortId(graph, opts.source, opts.sourcePort, "source");
    targetPort = resolvePortId(graph, opts.target, opts.targetPort, "target");
  }

  const link = new shapes.standard.Link({
    id: opts.id,
    source: { id: opts.source, port: sourcePort },
    target: { id: opts.target, port: targetPort },
    router: { name: "manhattan", args: { step: 10 } },
    connector: { name: "rounded", args: { radius: 4 } },
    attrs: {
      line: {
        stroke: colors.textMuted,
        strokeWidth: 1.5,
        targetMarker: {
          type: "path",
          d: "M 10 -5 0 0 10 5 Z",
          fill: colors.textMuted,
        },
      },
    },
  });

  if (opts.vertices && opts.vertices.length > 0) {
    link.vertices(opts.vertices);
  }

  return link;
}
