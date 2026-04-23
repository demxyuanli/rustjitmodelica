import type {
  AnnotationPoint,
  IconDiagramAnnotation,
  LineAnnotation,
} from "../components/diagramGraphicTypes";

/** Optional port metadata for connector-aware rendering (aligns with OMEdit port semantics). */
export interface PortHandleInfo {
  id: string;
  kind?: "input" | "output" | "flow" | "stream" | "signal";
  typeName?: string;
}

export interface DiagramNodeData {
  [key: string]: unknown;
  typeName: string;
  libraryId?: string;
  portHandles: string[];
  /** When set, overrides string-only portHandles for kind-aware UI. */
  portInfos?: PortHandleInfo[];
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
  replaceable?: boolean;
  constrainedbyType?: string;
  condition?: string;
  visible?: boolean;
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
  /** JointJS router name; mirrored to ConnectionData.line.routing when present. */
  router?: string;
}

export interface LayoutPoint {
  x: number;
  y: number;
}

export interface Transformation {
  origin?: AnnotationPoint;
  extent?: { p1: AnnotationPoint; p2: AnnotationPoint };
  rotation?: number;
}

export interface PlacementData {
  transformation?: Transformation;
  iconTransformation?: Transformation;
  visible?: boolean;
}

export interface ParamValue {
  name: string;
  value: string;
}

export interface ComponentData {
  name: string;
  typeName: string;
  libraryId?: string;
  placement?: PlacementData;
  icon?: IconDiagramAnnotation;
  rotation?: number;
  origin?: AnnotationPoint;
  params?: ParamValue[];
  connectorKind?: string;
  isInput?: boolean;
  isOutput?: boolean;
  replaceable?: boolean;
  constrainedbyType?: string;
  /** Parsed conditional component expression text (Modelica `if`). */
  condition?: string;
  visible?: boolean;
}

export interface ConnectionData {
  from: string;
  to: string;
  line?: LineAnnotation;
}

export interface DiagramModel {
  modelName: string;
  components: ComponentData[];
  connections: ConnectionData[];
  layout?: Record<string, LayoutPoint>;
  diagramAnnotation?: IconDiagramAnnotation;
  iconAnnotation?: IconDiagramAnnotation;
}
