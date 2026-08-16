import type { CatalogEntry, DiagramDocument, DiagramNode } from "../api/types";
import { createNodeFromCatalog } from "./connectionRules";
import { isDrawNode } from "./drawParams";
import { snapToGrid } from "./editOps";
import type { StudioFlowNode } from "./StudioBlockNode";

export function placeCatalogNode(
  entry: CatalogEntry,
  flowPos: { x: number; y: number },
  diagram: DiagramDocument,
  snapEnabled: boolean,
): { node: DiagramNode; flowNode: StudioFlowNode; next: DiagramDocument } {
  const pos = snapEnabled ? snapToGrid(flowPos.x, flowPos.y) : flowPos;
  const node = createNodeFromCatalog(entry, pos, diagram.nodes);
  const draw = isDrawNode(node);
  const flowNode: StudioFlowNode = {
    id: node.id,
    type: draw ? "studioDraw" : "studio",
    position: node.position,
    data: { diagram: node },
    zIndex: draw ? -1 : 0,
    style:
      draw && node.ui?.width != null && node.ui?.height != null
        ? { width: node.ui.width, height: node.ui.height }
        : undefined,
  };
  return {
    node,
    flowNode,
    next: { ...diagram, nodes: [...diagram.nodes, node] },
  };
}
