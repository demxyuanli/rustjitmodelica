import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type {
  CatalogEntry,
  DiagramDocument,
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
}): Promise<{
  success: boolean;
  report?: StudioReport | null;
  result?: SimulationResult | null;
  emittedCode?: string | null;
  runId?: string | null;
  cancelled?: boolean;
}> {
  return invoke("studio_simulate", { req: args });
}

export async function cancelSimulation(runId: string): Promise<boolean> {
  return invoke("studio_cancel_simulation", { runId });
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
  emitFromDiagram?: boolean;
}): Promise<import("./types").EmitArtifactsResult> {
  return invoke("studio_emit_artifacts", { req: args });
}

export async function pickFmuFile(): Promise<string | null> {
  return invoke("studio_pick_fmu_file");
}

export async function importFmu(args: {
  fmuPath: string;
  projectRoot?: string | null;
}): Promise<import("./types").FmuImportResult> {
  return invoke("studio_import_fmu", { req: args });
}

export async function listImportedFmus(rootDir: string): Promise<CatalogEntry[]> {
  return invoke("studio_list_imported_fmus", { rootDir });
}

export async function listenSimProgress(
  handler: (ev: StudioProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<StudioProgressEvent>("studio-sim-progress", (e) => handler(e.payload));
}
