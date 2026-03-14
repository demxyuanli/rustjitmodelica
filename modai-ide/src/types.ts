export interface JitValidateOptions {
  t_end?: number;
  dt?: number;
  atol?: number;
  rtol?: number;
  solver?: string;
  output_interval?: number;
}

export interface WarningItem {
  path: string;
  line: number;
  column: number;
  message: string;
}

export interface JitValidateResult {
  success: boolean;
  warnings: WarningItem[];
  errors: string[];
  state_vars: string[];
  output_vars: string[];
}

export interface SimulationResult {
  time: number[];
  series: Record<string, number[]>;
}

export interface EquationGraphNode {
  id: string;
  label: string;
  kind: string;
}

export interface EquationGraphEdge {
  source: string;
  target: string;
  kind: string;
}

export interface EquationGraph {
  nodes: EquationGraphNode[];
  edges: EquationGraphEdge[];
}

export interface DialogSelector {
  filter?: string;
  caption?: string;
}

export interface DialogAnnotation {
  tab?: string;
  group?: string;
  groupImage?: string;
  enable?: boolean;
  showStartAttribute?: boolean;
  connectorSizing?: boolean;
  colorSelector?: boolean;
  loadSelector?: DialogSelector;
  saveSelector?: DialogSelector;
}

export interface ComponentTypeParameter {
  name: string;
  typeName: string;
  defaultValue?: string;
  dialog?: DialogAnnotation;
  description?: string;
  group?: string;
  tab?: string;
  replaceable?: boolean;
}

export interface ComponentConnectorInfo {
  name: string;
  typeName: string;
  direction: "input" | "output" | "flow";
  description?: string;
  replaceable?: boolean;
}

export interface ComponentExampleInfo {
  title: string;
  description?: string;
  modelPath?: string;
  usage?: string;
}

export interface ComponentTypeInfo {
  name: string;
  qualifiedName?: string;
  kind: string;
  path?: string;
  libraryId?: string;
  libraryName?: string;
  libraryScope?: string;
  summary?: string;
  description?: string;
  usageHelp?: string;
  metadataSource?: string;
  extendsNames?: string[];
  connectors?: ComponentConnectorInfo[];
  examples?: ComponentExampleInfo[];
  parameters: ComponentTypeParameter[];
}

export interface ComponentTypeSource {
  qualifiedName: string;
  source: string;
  path?: string;
  libraryId: string;
  libraryName: string;
  libraryScope: string;
  content: string;
}

export interface InstantiableClass {
  name: string;
  qualifiedName: string;
  path?: string;
  source: string;
  kind: string;
  libraryId: string;
  libraryName: string;
  libraryScope: string;
  summary?: string;
  usageHelp?: string;
  exampleTitles?: string[];
}

export interface ComponentLibrary {
  id: string;
  scope: string;
  kind: string;
  displayName: string;
  sourcePath?: string;
  enabled: boolean;
  priority: number;
  builtIn: boolean;
  componentCount: number;
}

export interface ComponentLibraryTypeQueryResult {
  items: InstantiableClass[];
  total: number;
  hasMore: boolean;
}

export interface ComponentTypeRelationNode {
  id: string;
  label: string;
  kind: "component";
  typeName: string;
  x?: number;
  y?: number;
  isInput?: boolean;
  isOutput?: boolean;
}

export interface ComponentTypeRelationEdge {
  id: string;
  source: string;
  target: string;
  sourcePort?: string;
  targetPort?: string;
}

export interface ComponentTypeRelationGraph {
  modelName: string;
  nodes: ComponentTypeRelationNode[];
  edges: ComponentTypeRelationEdge[];
  unsupportedReason?: string;
}

export interface LayoutPoint {
  x: number;
  y: number;
}

export interface GraphicModelState<TAnnotation> {
  layout?: Record<string, LayoutPoint>;
  diagramAnnotation?: TAnnotation;
  iconAnnotation?: TAnnotation;
}

export interface GraphicalDocumentModel<TAnnotation = unknown, TComponent = unknown, TConnection = unknown> {
  modelName: string;
  components: TComponent[];
  connections: TConnection[];
  graphical: GraphicModelState<TAnnotation>;
}
