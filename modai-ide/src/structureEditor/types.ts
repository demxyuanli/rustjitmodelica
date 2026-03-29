import type {
  AnnotationPoint,
  IconDiagramAnnotation,
  LineAnnotation,
} from "../components/DiagramSvgRenderer";

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
