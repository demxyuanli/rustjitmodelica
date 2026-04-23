export interface JitValidateOptions {
  t_end?: number;
  dt?: number;
  atol?: number;
  rtol?: number;
  solver?: string;
  output_interval?: number;
  /** Stops validation after this tier: full | parse | flatten | analyze */
  validationTier?: string;
  /** Flattened parameter names for provenance impact probe (analysis only). */
  paramChangeImpactProbe?: string[];
  /** Flattened component instance path for provenance impact probe (analysis only). */
  instanceChangeImpactProbe?: string;
  /** Eq-expand parallel mode: off | guarded | on (default off). */
  eqExpandParallelMode?: "off" | "guarded" | "on";
}

export interface WarningItem {
  path: string;
  line: number;
  column: number;
  message: string;
}

/** Provenance impact summary (camelCase from backend). */
export interface JitProvenanceImpactAnalysis {
  affectedVars: string[];
  affectedEquationIndices: number[];
  requiresFullReflatten: boolean;
  reflattenReason?: string | null;
}

export interface JitProvenanceReport {
  equationCount: number;
  variableCount: number;
  parameterClosureCount: number;
  instanceCount: number;
  paramChangeImpact?: JitProvenanceImpactAnalysis | null;
  instanceChangeImpact?: JitProvenanceImpactAnalysis | null;
  incrementalCodegenWorthwhileHint?: boolean | null;
}

export interface JitValidateResult {
  schema_version?: string;
  success: boolean;
  warnings: WarningItem[];
  errors: string[];
  diagnostics?: DiagnosticErrorItem[];
  state_vars: string[];
  output_vars: string[];
  /** Phase timings and counts from the compiler (shown in Simulation Output tab). */
  compile_trace?: string[];
  validation_stop_phase?: string | null;
  validation_partial?: boolean;
  provenance?: JitProvenanceReport | null;
}

export interface DiagnosticErrorItem {
  code: string;
  message: string;
  path?: string | null;
  line?: number | null;
  column?: number | null;
}

export interface JitApiMeta {
  schemaVersion: string;
  operation: string;
}

export interface JitApiError {
  code: string;
  message: string;
  path?: string | null;
  line?: number | null;
  column?: number | null;
}

export interface JitApiEnvelope<T> {
  ok: boolean;
  meta: JitApiMeta;
  data?: T | null;
  errors: JitApiError[];
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
  truncated?: boolean;
  totalEquations?: number;
  includedEquations?: number;
  omittedEquations?: number;
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
  /** Resolved `.mo` path relative to project root when the file is under the project directory. */
  projectRelativePath?: string;
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
  sourceUrl?: string;
  sourceRef?: string;
}

export interface LibrarySuggestion {
  displayName: string;
  url: string;
  refName: string;
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

export type RegressionPlanStrategy = "category" | "feature" | "relation";
export type RegressionWorkspaceMode = "persistent" | "ephemeral";
export type RegressionWorkspaceStatus = "planned" | "running" | "completed" | "failed" | "cancelled";

export interface RegressionPlanRequest {
  strategy: RegressionPlanStrategy;
  categories: string[];
  featureIds: string[];
  changedFiles: string[];
  includeIndirect: boolean;
  maxCases?: number | null;
  workspaceMode: RegressionWorkspaceMode;
  includeModelicaExamples: boolean;
  includeModelicaTest: boolean;
}

export interface RegressionPlannedCase {
  name: string;
  reason: string;
  priority: number;
  category: string;
}

export interface RegressionExecutionPlanState {
  strategy: RegressionPlanStrategy;
  changedSources: string[];
  affectedFeatures: string[];
  plannedCases: RegressionPlannedCase[];
  skippedCases: string[];
}

export interface RegressionRunRecord {
  timestamp: string;
  caseType: string;
  caseName: string;
  durationMs: number;
  expectTargetOk: boolean;
  actualOk: boolean;
  exitCode: number;
  status: string;
  reason:
    | "expectationMet"
    | "modelNotFound"
    | "dependencyMissing"
    | "newtonNonconverged"
    | "parseError"
    | "runtimeError"
    | "timeout"
    | "processError"
    | "cancelled";
  detail: string;
}

export interface RegressionWorkspaceInfo {
  workspaceId: string;
  workspacePath: string;
  strategy: RegressionPlanStrategy;
  status: RegressionWorkspaceStatus;
  createdAt: string;
}

export interface RegressionWorkspaceRunResult {
  total: number;
  passed: number;
  failed: number;
  durationMs: number;
}

export interface RegressionWorkspaceState {
  info: RegressionWorkspaceInfo;
  plan: RegressionExecutionPlanState;
  result?: RegressionWorkspaceRunResult | null;
  records: RegressionRunRecord[];
}
