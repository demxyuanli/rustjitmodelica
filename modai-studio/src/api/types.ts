export interface Position {
  x: number;
  y: number;
}

export interface ParamValue {
  name: string;
  value: string;
}

export interface PortDef {
  id: string;
  kind: string;
  direction: string;
  typeName: string;
}

export interface DiagramNode {
  id: string;
  instanceName: string;
  typeName: string;
  domain: string;
  catalogId: string;
  position: Position;
  rotation: number;
  params: ParamValue[];
  ports: PortDef[];
  nodeKind: string;
  subsystemRef: string | null;
  plotHints: string[];
  ui?: {
    label?: string;
    width?: number;
    height?: number;
    portBindings?: Record<string, { nodeId: string; portId: string }>;
    fmuAsset?: string;
    fmuGuid?: string;
    fmuModelIdentifier?: string;
    /** Modelica Icon annotation for block face. */
    icon?: unknown;
    /** Draw instance name below the block body. */
    labelBelow?: boolean;
    /** Relative .mo asset for Model Reference / userlib (e.g. wrapped/Foo.mo). */
    moAsset?: string;
    /** Variant Subsystem choices (F87); active one is mirrored in subsystemRef. */
    variants?: { id: string; label: string; subsystemRef: string }[];
    activeVariantId?: string;
    /** Param names pinned to the pre-run dashboard. */
    dashboardPins?: string[];
  };
}

export interface DiagramEdge {
  id: string;
  sourceNodeId: string;
  sourcePortId: string;
  targetNodeId: string;
  targetPortId: string;
  signalName?: string | null;
  /** Physical / signal unit annotation (display + docs). */
  unit?: string | null;
  /** Array / vector dimension hint, e.g. "1", "3", "[2]". */
  dimension?: string | null;
  vertices?: Position[];
  routing?: string;
}

export interface DiagramDocument {
  schemaVersion: number;
  modelName: string;
  diagramId: string;
  parentDiagramId: string | null;
  nodes: DiagramNode[];
  edges: DiagramEdge[];
  annotations: unknown[];
  viewport?: { x: number; y: number; zoom: number };
}

export interface SimulationDefaults {
  stopTime: number;
  numberOfIntervals: number;
  solver: string;
  tolerance: number;
  maxStep?: number | null;
  minStep?: number | null;
  loggedVariables: string[];
}

/** Project-level parameter dictionary (F85). Block params use `$Name` refs. */
export interface ParamDictionaryEntry {
  name: string;
  value: string;
  description?: string;
}

export interface StudioProjectManifest {
  schemaVersion: number;
  product: string;
  modelName: string;
  libraryPaths: string[];
  simulationDefaults: SimulationDefaults;
  ui: { locale: string };
  /** F85 data dictionary entries. */
  paramDictionary?: ParamDictionaryEntry[];
}

export interface CatalogEntry {
  id: string;
  typeName: string;
  domain: string;
  category: string;
  displayKey: string;
  defaultInstancePrefix: string;
  requiresLibrary: string;
  defaultParams: {
    name: string;
    value: string;
    valueType?: string;
    unit?: string | null;
    description?: string | null;
  }[];
  ports: PortDef[];
  tags: string[];
  plotHints: string[];
  fmuAsset?: string | null;
  fmuGuid?: string | null;
  fmuModelIdentifier?: string | null;
  /** Modelica Icon annotation JSON (from wrap .mo). */
  icon?: unknown | null;
}

export type DiagnosticSource = "compiler" | "advisor" | "connection" | "structure";

export interface StudioDiagnostic {
  message: string;
  path?: string | null;
  line?: number | null;
  column?: number | null;
  source?: DiagnosticSource;
  rule?: string;
  hint?: string;
  nodeId?: string;
  severity?: "error" | "warning" | "info";
  params?: Record<string, string | number>;
}

export interface StudioReport {
  success: boolean;
  stage: string;
  errors: StudioDiagnostic[];
  warnings: StudioDiagnostic[];
}

export interface SimulationResult {
  time: number[];
  series: Record<string, number[]>;
}

export interface OpenProjectResult {
  rootDir: string;
  manifest: StudioProjectManifest;
  diagram: DiagramDocument;
  modelSource: string;
  subsystems?: Record<string, DiagramDocument>;
}

export interface ExampleInfo {
  id: string;
  title: string;
  modelName: string;
  domain: string;
  canSimulate: boolean;
  rootDir: string;
}

export interface StudioSimOptions {
  stopTime: number;
  numberOfIntervals: number;
  solver: string;
  tolerance: number;
}

/** Wall-clock phase breakdown from studio_simulate (F89). */
export interface SimProfile {
  emitMs: number;
  compileMs: number;
  applyParamsMs: number;
  integrateMs: number;
  totalMs: number;
  compileSkipped: boolean;
  restartMode?: string | null;
}

export interface RunRecord {
  runId: string;
  createdAt: string;
  label: string;
  success: boolean;
  options: StudioSimOptions;
  result: SimulationResult;
  wallMs?: number;
  profile?: SimProfile;
  paramSnapshot?: Record<string, string>;
}

export type ExportMode = "coder" | "fmu-cs" | "fmu-me";

/** Options collected by ExportWizard / last-export prefs. */
export interface EmitOptions {
  mode: ExportMode;
  outDir: string;
  coderPackage: boolean;
  modelIdentifier?: string;
  guid?: string;
}

export interface EmitRecord {
  id: string;
  createdAt: string;
  modelName: string;
  mode: ExportMode;
  packageKind: string;
  outDir: string;
  fmuPath?: string | null;
  files: string[];
  success: boolean;
  wallMs?: number;
}

export interface EmitArtifactsResult {
  success: boolean;
  outDir: string;
  files: string[];
  fmuPath?: string | null;
  report?: StudioReport | null;
  /** "c" | "coder" | "fmu" | "fmu-me" | "coder+fmu" | "coder+fmu-me" */
  packageKind?: string | null;
}

export interface FmuImportResult {
  success: boolean;
  error?: string | null;
  catalogEntry?: CatalogEntry | null;
  assetDir: string;
  modelName: string;
  guid: string;
  modelIdentifier: string;
  fmiVersion: string;
  hasCoSimulation: boolean;
  hasBinary: boolean;
  scalars: {
    name: string;
    valueReference: number;
    causality: string;
    variability: string;
    start?: number | null;
  }[];
}
