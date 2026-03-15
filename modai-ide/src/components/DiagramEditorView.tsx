import { useEffect, useRef, useState } from "react";
import { dia, shapes } from "@joint/core";
import {
  createElement,
  createLink,
  determineNodeShape,
  type NodeShape,
} from "./diagram/jointShapes";
import {
  createPaper,
  createPaperHandle,
  resolveDiagramColors,
  type JointPaperHandle,
} from "../utils/jointUtils";
import { useDiagramScheme } from "../contexts/DiagramSchemeContext";
import { JointMiniMap } from "./diagram/JointMiniMap";
import type { AnnotationPoint, IconDiagramAnnotation } from "./DiagramSvgRenderer";

export interface DiagramNodeData {
  [key: string]: unknown;
  typeName: string;
  libraryId?: string;
  portHandles: string[];
  icon?: IconDiagramAnnotation;
  rotation?: number;
  params?: { name: string; value: string }[];
  connectorKind?: string;
  isInput?: boolean;
  isOutput?: boolean;
  isSourceNode?: boolean;
  isSinkNode?: boolean;
  onDoubleClick?: (typeName: string, libraryId?: string) => void;
  simValues?: Record<string, number>;
  hasError?: boolean;
  errorMessage?: string;
}

export interface DiagramNode {
  id: string;
  position: { x: number; y: number };
  data: DiagramNodeData;
  selected?: boolean;
}

export interface DiagramLink {
  id: string;
  source: string;
  sourcePort: string;
  target: string;
  targetPort: string;
  vertices?: AnnotationPoint[];
}

const DEFAULT_HANDLE_ID = "__default__";

export interface DiagramEditorViewProps {
  nodes: DiagramNode[];
  links: DiagramLink[];
  readOnly: boolean;
  onNodePositionChange?: (id: string, position: { x: number; y: number }) => void;
  onConnect?: (source: string, sourcePort: string, target: string, targetPort: string) => void;
  onDeleteElements?: (nodeIds: string[], linkIds: string[]) => void;
  onSelectNode?: (id: string | null) => void;
  onPaperReady?: (handle: JointPaperHandle) => void;
  showMiniMap?: boolean;
  snapToGrid?: boolean;
}

function getNodeShape(data: DiagramNodeData): NodeShape {
  return determineNodeShape({
    isInput: data.isInput,
    isOutput: data.isOutput,
    isSourceNode: data.isSourceNode,
    isSinkNode: data.isSinkNode,
  });
}

export function DiagramEditorView({
  nodes,
  links,
  readOnly,
  onNodePositionChange,
  onConnect,
  onDeleteElements,
  onSelectNode,
  onPaperReady,
  showMiniMap = true,
  snapToGrid = true,
}: DiagramEditorViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const graphRef = useRef<dia.Graph | null>(null);
  const paperRef = useRef<dia.Paper | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [hoveredId, setHoveredId] = useState<string | null>(null);
  const nodesRef = useRef(nodes);
  nodesRef.current = nodes;
  const linksRef = useRef(links);
  linksRef.current = links;

  const [, forceUpdate] = useState(0);
  const { schemeId } = useDiagramScheme();

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const paperEl = document.createElement("div");
    paperEl.style.width = "100%";
    paperEl.style.height = "100%";
    container.appendChild(paperEl);

    const graph = new dia.Graph({}, { cellNamespace: shapes });
    const paper = createPaper({
      el: paperEl,
      graph,
      gridSize: snapToGrid ? 10 : 1,
      readOnly,
      onScale: () => forceUpdate((v) => v + 1),
      onTranslate: () => forceUpdate((v) => v + 1),
    });

    graphRef.current = graph;
    paperRef.current = paper;

    paper.on("element:pointerclick", (elementView: dia.ElementView) => {
      const elId = elementView.model.id as string;
      setSelectedId(elId);
      onSelectNode?.(elId);
    });

    paper.on("blank:pointerclick", () => {
      setSelectedId(null);
      onSelectNode?.(null);
    });

    paper.on("element:mouseenter", (elementView: dia.ElementView) => {
      setHoveredId(elementView.model.id as string);
    });

    paper.on("element:mouseleave", () => {
      setHoveredId(null);
    });

    paper.on("element:pointerup", (elementView: dia.ElementView) => {
      const el = elementView.model as dia.Element;
      const pos = el.position();
      onNodePositionChange?.(el.id as string, { x: pos.x, y: pos.y });
    });

    paper.on("link:connect", (linkView: dia.LinkView) => {
      const link = linkView.model;
      const src = link.source();
      const tgt = link.target();
      if (src.id && tgt.id) {
        onConnect?.(
          src.id as string,
          (src.port as string) || DEFAULT_HANDLE_ID,
          tgt.id as string,
          (tgt.port as string) || DEFAULT_HANDLE_ID
        );
        link.remove();
      }
    });

    paper.on("element:pointerdblclick", (elementView: dia.ElementView) => {
      const elId = elementView.model.id as string;
      const node = nodesRef.current.find((n) => n.id === elId);
      if (node?.data.onDoubleClick) {
        node.data.onDoubleClick(node.data.typeName, node.data.libraryId as string | undefined);
      }
    });

    const handle = createPaperHandle(paper);
    onPaperReady?.(handle);

    syncGraphFromProps(graph, nodesRef.current, linksRef.current);

    if (graph.getElements().length > 0) {
      try {
        paper.transformToFitContent({ padding: 30, maxScale: 1.5, minScale: 0.1 });
      } catch (_) {
        // empty graph
      }
    }

    forceUpdate((v) => v + 1);

    return () => {
      paper.remove();
      graphRef.current = null;
      paperRef.current = null;
    };
  }, []);

  useEffect(() => {
    const graph = graphRef.current;
    if (!graph) return;
    syncGraphFromProps(graph, nodes, links);
  }, [nodes, links]);

  useEffect(() => {
    const graph = graphRef.current;
    if (!graph) return;
    const colors = resolveDiagramColors();

    graph.getElements().forEach((el) => {
      const elId = el.id as string;
      const isSelected = elId === selectedId;
      const isHovered = elId === hoveredId;

      if (isSelected) {
        el.attr("body/stroke", colors.primary);
        el.attr("body/strokeWidth", 2);
      } else if (isHovered) {
        el.attr("body/stroke", colors.primary);
        el.attr("body/strokeWidth", 1.5);
      } else {
        const node = nodes.find((n) => n.id === elId);
        const hasError = node?.data?.hasError;
        el.attr("body/stroke", hasError ? "#ef4444" : colors.border);
        el.attr("body/strokeWidth", 1);
      }
    });
  }, [selectedId, hoveredId, nodes, schemeId]);

  useEffect(() => {
    if (!readOnly) {
      const onKeyDown = (e: KeyboardEvent) => {
        if (e.key === "Delete" && selectedId) {
          const graph = graphRef.current;
          if (!graph) return;
          const cell = graph.getCell(selectedId);
          if (cell) {
            const nodeIds: string[] = [];
            const linkIds: string[] = [];
            if (cell.isElement()) {
              nodeIds.push(selectedId);
              const connectedLinks = graph.getConnectedLinks(cell as dia.Element);
              connectedLinks.forEach((l) => linkIds.push(l.id as string));
            } else {
              linkIds.push(selectedId);
            }
            onDeleteElements?.(nodeIds, linkIds);
            setSelectedId(null);
            onSelectNode?.(null);
          }
        }
      };
      window.addEventListener("keydown", onKeyDown);
      return () => window.removeEventListener("keydown", onKeyDown);
    }
  }, [readOnly, selectedId, onDeleteElements, onSelectNode]);

  return (
    <div className="relative w-full h-full overflow-hidden">
      <div ref={containerRef} className="absolute inset-0" />
      {showMiniMap && (
        <JointMiniMap
          paper={paperRef.current}
          graph={graphRef.current}
        />
      )}
    </div>
  );
}

function syncGraphFromProps(
  graph: dia.Graph,
  nodes: DiagramNode[],
  links: DiagramLink[]
) {
  const existingElements = new Map<string, dia.Element>();
  graph.getElements().forEach((el) => existingElements.set(el.id as string, el));

  const existingLinks = new Map<string, dia.Link>();
  graph.getLinks().forEach((l) => existingLinks.set(l.id as string, l));

  const nodeIds = new Set(nodes.map((n) => n.id));
  const linkIds = new Set(links.map((l) => l.id));

  existingElements.forEach((el, id) => {
    if (!nodeIds.has(id)) {
      el.remove();
    }
  });
  existingLinks.forEach((l, id) => {
    if (!linkIds.has(id)) {
      l.remove();
    }
  });

  for (const node of nodes) {
    const existing = existingElements.get(node.id);
    if (existing) {
      const pos = existing.position();
      if (Math.abs(pos.x - node.position.x) > 1 || Math.abs(pos.y - node.position.y) > 1) {
        existing.position(node.position.x, node.position.y);
      }
    } else {
      const shape = getNodeShape(node.data);
      const ports = node.data.portHandles?.length
        ? node.data.portHandles
        : [DEFAULT_HANDLE_ID];
      const paramStr = node.data.params
        ?.filter((item) => item.value)
        .map((item) => (item.name ? `${item.name}=${item.value}` : item.value))
        .join(", ");

      const el = createElement({
        id: node.id,
        shape,
        position: node.position,
        label: node.id,
        sublabel: node.data.typeName,
        paramStr,
        ports,
        connectorKind: node.data.connectorKind,
        hasError: node.data.hasError,
        errorMessage: node.data.errorMessage,
      });
      graph.addCell(el);
    }
  }

  for (const link of links) {
    if (!existingLinks.has(link.id)) {
      const l = createLink(
        {
          id: link.id,
          source: link.source,
          sourcePort: link.sourcePort,
          target: link.target,
          targetPort: link.targetPort,
          vertices: link.vertices,
        },
        graph
      );
      graph.addCell(l);
    }
  }
}
