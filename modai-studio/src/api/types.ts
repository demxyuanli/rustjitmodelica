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
  };
}

export interface DiagramEdge {
  id: string;
  sourceNodeId: string;
  sourcePortId: string;
  targetNodeId: string;
  targetPortId: string;
  signalName?: string | null;
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

export interface StudioProjectManifest {
  schemaVersion: number;
  product: string;
  modelName: string;
  libraryPaths: string[];
  simulationDefaults: SimulationDefaults;
  ui: { locale: string };
}

export interface CatalogEntry {
  id: string;
  typeName: string;
  domain: string;
  category: string;
  displayKey: string;
  defaultInstancePrefix: string;
  requiresLibrary: string;
  defaultParams: { name: string; value: string; valueType?: string }[];
  ports: PortDef[];
  tags: string[];
  plotHints: string[];
  fmuAsset?: string | null;
  fmuGuid?: string | null;
  fmuModelIdentifier?: string | null;
}

export interface StudioDiagnostic {
  message: string;
  path?: string | null;
  line?: number | null;
  column?: number | null;
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

export interface StudioSimOptions {
  stopTime: number;
  numberOfIntervals: number;
  solver: string;
  tolerance: number;
}

export interface RunRecord {
  runId: string;
  createdAt: string;
  label: string;
  success: boolean;
  options: StudioSimOptions;
  result: SimulationResult;
  wallMs?: number;
}

export interface EmitArtifactsResult {
  success: boolean;
  outDir: string;
  files: string[];
  fmuPath?: string | null;
  report?: StudioReport | null;
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
