import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type {
  CatalogEntry,
  DiagramDocument,
  ExampleInfo,
  OpenProjectResult,
  SimulationResult,
  StudioProjectManifest,
  StudioReport,
  StudioSimOptions,
} from "./types";

export interface StudioProgressEvent {
  runId: string;
  phase: string;
  simTime?: number | null;
  tEnd?: number | null;
  step?: number | null;
  fraction?: number | null;
  message: string;
  liveTime?: number[] | null;
  liveSeries?: Record<string, number[]> | null;
}

export async function catalogList(rootDir?: string | null): Promise<CatalogEntry[]> {
  return invoke("studio_catalog_list", { rootDir: rootDir ?? null });
}

export async function projectCreate(rootDir: string, modelName: string): Promise<OpenProjectResult> {
  return invoke("studio_project_create", { rootDir, modelName });
}

export async function projectOpen(rootDir: string): Promise<OpenProjectResult> {
  return invoke("studio_project_open", { rootDir });
}

/** Startup auto-open path from MODAI_STUDIO_OPEN or --project=. */
export async function startupProjectPath(): Promise<string | null> {
  return invoke("studio_startup_project_path");
}

export async function listExamples(): Promise<ExampleInfo[]> {
  return invoke("studio_list_examples");
}

export async function openExample(id: string): Promise<OpenProjectResult> {
  return invoke("studio_open_example", { id });
}

export async function projectSave(
  rootDir: string,
  manifest: StudioProjectManifest,
  diagram: DiagramDocument,
  subsystems: Record<string, DiagramDocument> = {},
): Promise<string> {
  return invoke("studio_project_save", { req: { rootDir, manifest, diagram, subsystems } });
}

export async function pickDirectory(): Promise<string | null> {
  return invoke("studio_pick_directory");
}

export async function validateModel(args: {
  rootDir?: string;
  modelName: string;
  code?: string;
  diagram?: DiagramDocument;
  subsystems?: Record<string, DiagramDocument>;
  libraryPaths: string[];
  options: StudioSimOptions;
  emitFromDiagram?: boolean;
  /** When true, Full compile including JIT; default Analyze-only. */
  fullJit?: boolean;
  paramDictionary?: import("./types").ParamDictionaryEntry[];
}): Promise<StudioReport> {
  return invoke("studio_validate", { req: args });
}

export async function simulateModel(args: {
  rootDir?: string;
  modelName: string;
  code?: string;
  diagram?: DiagramDocument;
  subsystems?: Record<string, DiagramDocument>;
  libraryPaths: string[];
  options: StudioSimOptions;
  emitFromDiagram?: boolean;
  /** Reuse compiled Artifacts when only params / sim options change (default true). */
  fastRestart?: boolean;
  paramDictionary?: import("./types").ParamDictionaryEntry[];
}): Promise<{
  success: boolean;
  report?: StudioReport | null;
  result?: SimulationResult | null;
  emittedCode?: string | null;
  runId?: string | null;
  cancelled?: boolean;
  /** "fastRestart" | "fullCompile" when applicable */
  restartMode?: string | null;
  profile?: import("./types").SimProfile | null;
}> {
  return invoke("studio_simulate", { req: args });
}

export async function cancelSimulation(runId: string): Promise<boolean> {
  return invoke("studio_cancel_simulation", { runId });
}

export async function pauseSimulation(runId: string): Promise<boolean> {
  return invoke("studio_pause_simulation", { runId });
}

export async function resumeSimulation(runId: string): Promise<boolean> {
  return invoke("studio_resume_simulation", { runId });
}

export async function stepSimulation(runId: string): Promise<boolean> {
  return invoke("studio_step_simulation", { runId });
}

export async function simIsPaused(runId: string): Promise<boolean> {
  return invoke("studio_sim_is_paused", { runId });
}

export async function pickExportDirectory(): Promise<string | null> {
  return invoke("studio_pick_export_directory");
}

export async function emitArtifacts(args: {
  rootDir?: string;
  modelName: string;
  code?: string;
  diagram?: DiagramDocument;
  subsystems?: Record<string, DiagramDocument>;
  libraryPaths: string[];
  options: StudioSimOptions;
  outDir: string;
  packageFmu?: boolean;
  /** Default true: write rml_step / CMake / interface.json. */
  coderPackage?: boolean;
  /** "cs" | "me" when packaging FMU. */
  fmiKind?: string;
  modelIdentifier?: string;
  guid?: string;
  emitFromDiagram?: boolean;
  paramDictionary?: import("./types").ParamDictionaryEntry[];
}): Promise<import("./types").EmitArtifactsResult> {
  return invoke("studio_emit_artifacts", { req: args });
}

export async function pickFmuFile(): Promise<string | null> {
  return invoke("studio_pick_fmu_file");
}

export async function pickMoFile(): Promise<string | null> {
  return invoke("studio_pick_mo_file");
}

export async function pickDataFile(): Promise<string | null> {
  return invoke("studio_pick_data_file");
}

export async function readTextFile(path: string): Promise<string> {
  return invoke("studio_read_text_file", { path });
}

export async function importFmu(args: {
  fmuPath: string;
  projectRoot?: string | null;
}): Promise<import("./types").FmuImportResult> {
  return invoke("studio_import_fmu", { req: args });
}

export interface WrapMoResult {
  success: boolean;
  error?: string | null;
  catalogEntry?: CatalogEntry | null;
  modelName: string;
  typeName: string;
  assetPath: string;
  inputCount: number;
  outputCount: number;
  connectorCount: number;
  paramCount: number;
  hasIcon: boolean;
}

export async function wrapMo(args: {
  moPath: string;
  projectRoot?: string | null;
}): Promise<WrapMoResult> {
  return invoke("studio_wrap_mo", { req: args });
}

export interface ImportPackResult {
  success: boolean;
  error?: string | null;
  packName: string;
  libraryPath: string;
  entries: CatalogEntry[];
  copied: number;
}

export interface ExportPackResult {
  success: boolean;
  error?: string | null;
  outDir: string;
  copied: number;
}

export async function pickCatalogPackDirectory(): Promise<string | null> {
  return invoke("studio_pick_catalog_pack_directory");
}

export async function importCatalogPack(args: {
  packDir: string;
  projectRoot: string;
}): Promise<ImportPackResult> {
  return invoke("studio_import_catalog_pack", { req: args });
}

export async function exportCatalogPack(args: {
  projectRoot: string;
  relativeAssets?: string[];
  outDir: string;
  packName?: string;
}): Promise<ExportPackResult> {
  return invoke("studio_export_catalog_pack", { req: args });
}

export async function listImportedFmus(rootDir: string): Promise<CatalogEntry[]> {
  return invoke("studio_list_imported_fmus", { rootDir });
}

export async function listenSimProgress(
  handler: (ev: StudioProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<StudioProgressEvent>("studio-sim-progress", (e) => handler(e.payload));
}
