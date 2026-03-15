import { invoke } from "@tauri-apps/api/core";
import type {
  ComponentLibrary,
  ComponentLibraryTypeQueryResult,
  ComponentTypeInfo,
  ComponentTypeRelationGraph,
  ComponentTypeSource,
  EquationGraph,
  GraphicalDocumentModel,
  InstantiableClass,
  JitValidateOptions,
  JitValidateResult,
  SimulationResult,
} from "../types";

// --- JIT / simulation ---

export interface JitValidateRequest {
  code: string;
  modelName: string;
  options: JitValidateOptions;
  projectDir?: string;
}

export interface RunSimulationRequest {
  code: string;
  modelName: string;
  options: JitValidateOptions;
  projectDir?: string;
}

export async function jitValidate(request: JitValidateRequest): Promise<JitValidateResult> {
  return invoke<JitValidateResult>("jit_validate", { request });
}

export async function runSimulation(request: RunSimulationRequest): Promise<SimulationResult> {
  return invoke<SimulationResult>("run_simulation_cmd", { request });
}

export async function getEquationGraph(
  code: string,
  modelName: string,
  projectDir?: string | null
): Promise<EquationGraph> {
  return invoke<EquationGraph>("get_equation_graph", {
    code,
    modelName,
    projectDir: projectDir ?? undefined,
  });
}

// --- Project / Modelica files ---

export async function openProjectDir(): Promise<string | null> {
  return invoke<string | null>("open_project_dir");
}

export async function reopenProjectDir(path: string): Promise<string> {
  return invoke<string>("reopen_project_dir", { path });
}

export async function pickComponentLibraryFolder(): Promise<string | null> {
  return invoke<string | null>("pick_component_library_folder");
}

export async function pickComponentLibraryFiles(): Promise<string[]> {
  return invoke<string[]>("pick_component_library_files");
}

export async function listMoTree(projectDir: string) {
  return invoke("list_mo_tree", { projectDir });
}

export async function listMoFiles(projectDir: string) {
  return invoke<string[]>("list_mo_files", { projectDir });
}

export async function listInstantiableClasses(projectDir?: string | null): Promise<InstantiableClass[]> {
  return invoke<InstantiableClass[]>("list_instantiable_classes", { projectDir: projectDir ?? undefined });
}

export async function queryComponentLibraryTypes(params: {
  projectDir?: string | null;
  libraryId?: string | null;
  scope?: string | null;
  enabledOnly?: boolean;
  query?: string;
  offset?: number;
  limit?: number;
}): Promise<ComponentLibraryTypeQueryResult> {
  return invoke<ComponentLibraryTypeQueryResult>("query_component_library_types", {
    projectDir: params.projectDir ?? undefined,
    libraryId: params.libraryId ?? undefined,
    scope: params.scope ?? undefined,
    enabledOnly: params.enabledOnly ?? true,
    query: params.query ?? "",
    offset: params.offset ?? 0,
    limit: params.limit ?? 100,
  });
}

export async function listComponentLibraries(projectDir?: string | null): Promise<ComponentLibrary[]> {
  return invoke<ComponentLibrary[]>("list_component_libraries", { projectDir: projectDir ?? undefined });
}

export async function addComponentLibrary(params: {
  projectDir?: string | null;
  scope: string;
  kind: string;
  sourcePath: string;
  displayName?: string;
}): Promise<ComponentLibrary> {
  return invoke<ComponentLibrary>("add_component_library", {
    projectDir: params.projectDir ?? undefined,
    scope: params.scope,
    kind: params.kind,
    sourcePath: params.sourcePath,
    displayName: params.displayName,
  });
}

export async function removeComponentLibrary(params: {
  projectDir?: string | null;
  scope: string;
  libraryId: string;
}): Promise<void> {
  await invoke("remove_component_library", {
    projectDir: params.projectDir ?? undefined,
    scope: params.scope,
    libraryId: params.libraryId,
  });
}

export async function setComponentLibraryEnabled(params: {
  projectDir?: string | null;
  scope: string;
  libraryId: string;
  enabled: boolean;
}): Promise<ComponentLibrary> {
  return invoke<ComponentLibrary>("set_component_library_enabled", {
    projectDir: params.projectDir ?? undefined,
    scope: params.scope,
    libraryId: params.libraryId,
    enabled: params.enabled,
  });
}

export async function getComponentTypeDetails(
  projectDir: string | null | undefined,
  typeName: string,
  libraryId?: string | null,
): Promise<ComponentTypeInfo> {
  return invoke<ComponentTypeInfo>("get_component_type_details", {
    projectDir: projectDir ?? undefined,
    typeName,
    libraryId: libraryId ?? undefined,
  });
}

export async function readComponentTypeSource(
  projectDir: string | null | undefined,
  typeName: string,
  libraryId?: string | null,
): Promise<ComponentTypeSource> {
  return invoke<ComponentTypeSource>("read_component_type_source", {
    projectDir: projectDir ?? undefined,
    typeName,
    libraryId: libraryId ?? undefined,
  });
}

export async function getComponentTypeRelationGraph(
  projectDir: string | null | undefined,
  typeName: string,
  libraryId?: string | null,
): Promise<ComponentTypeRelationGraph> {
  return invoke<ComponentTypeRelationGraph>("get_component_type_relation_graph", {
    projectDir: projectDir ?? undefined,
    typeName,
    libraryId: libraryId ?? undefined,
  });
}

export async function getGraphicalDocumentFromSource<TAnnotation = unknown, TComponent = unknown, TConnection = unknown>(
  source: string,
  projectDir?: string | null,
  relativePath?: string | null,
): Promise<GraphicalDocumentModel<TAnnotation, TComponent, TConnection>> {
  return invoke<GraphicalDocumentModel<TAnnotation, TComponent, TConnection>>("get_graphical_document_from_source", {
    source,
    projectDir: projectDir ?? undefined,
    relativePath: relativePath ?? undefined,
  });
}

export async function applyGraphicalDocumentEdits<TAnnotation = unknown, TComponent = unknown, TConnection = unknown>(
  source: string,
  document: GraphicalDocumentModel<TAnnotation, TComponent, TConnection>,
  projectDir?: string | null,
  relativePath?: string | null,
): Promise<{ newSource: string }> {
  return invoke<{ newSource: string }>("apply_graphical_document_edits", {
    source,
    document,
    projectDir: projectDir ?? undefined,
    relativePath: relativePath ?? undefined,
  });
}

export async function readProjectFile(projectDir: string, relativePath: string): Promise<string> {
  return invoke<string>("read_project_file", { projectDir, relativePath });
}

export async function writeProjectFile(projectDir: string, relativePath: string, content: string): Promise<void> {
  await invoke("write_project_file", { projectDir, relativePath, content });
}

// --- Git helpers ---

export interface GitStatus {
  branch?: string;
  modified?: string[];
  staged?: string[];
}

export async function gitIsRepo(projectDir: string): Promise<boolean> {
  return invoke<boolean>("git_is_repo", { projectDir });
}

export async function gitStatus(projectDir: string): Promise<GitStatus> {
  return invoke<GitStatus>("git_status", { projectDir });
}

export async function gitInit(projectDir: string): Promise<void> {
  await invoke("git_init", { projectDir });
}

export async function gitStage(projectDir: string, paths: string[]): Promise<void> {
  await invoke("git_stage", { projectDir, paths });
}

export async function gitUnstage(projectDir: string, paths: string[]): Promise<void> {
  await invoke("git_unstage", { projectDir, paths });
}

export async function gitCommit(projectDir: string, message: string): Promise<void> {
  await invoke("git_commit", { projectDir, message });
}

export async function gitLog(projectDir: string, relativePath?: string, limit?: number) {
  return invoke("git_log", { projectDir, relativePath, limit });
}

export async function gitLogGraph(projectDir: string, limit?: number) {
  return invoke("git_log_graph", { projectDir, limit });
}

export async function gitDiffFile(
  projectDir: string,
  relativePath: string,
  base?: string,
): Promise<string> {
  return invoke<string>("git_diff_file", { projectDir, relativePath, base });
}

export async function gitDiffFileStaged(projectDir: string, relativePath: string): Promise<string> {
  return invoke<string>("git_diff_file_staged", { projectDir, relativePath });
}

export async function gitShowFile(
  projectDir: string,
  revision: string,
  relativePath: string,
): Promise<string> {
  return invoke<string>("git_show_file", { projectDir, revision, relativePath });
}

// --- Index / search ---

export async function indexRepoRoot(): Promise<string> {
  return invoke<string>("index_repo_root");
}

export async function indexBuild(projectDir: string) {
  return invoke("index_build", { projectDir });
}

export async function indexBuildRepo() {
  return invoke("index_build_repo");
}

export async function indexRefresh(projectDir: string) {
  return invoke("index_refresh", { projectDir });
}

export async function indexRebuild(projectDir: string) {
  return invoke("index_rebuild", { projectDir });
}

export async function indexRefreshRepo() {
  return invoke("index_refresh_repo");
}

export async function indexRebuildRepo() {
  return invoke("index_rebuild_repo");
}

export async function indexStats(projectDir: string) {
  return invoke("index_stats", { projectDir });
}

export async function indexUpdateFile(projectDir: string, filePath: string): Promise<void> {
  await invoke("index_update_file", { projectDir, filePath });
}

export async function indexStartWatcher(projectDir: string): Promise<void> {
  await invoke("index_start_watcher", { projectDir });
}

export async function indexStopWatcher(): Promise<void> {
  await invoke("index_stop_watcher");
}

export async function indexSearchSymbols(
  projectDir: string,
  query: string,
  kind?: string,
  limit?: number,
) {
  return invoke("index_search_symbols", { projectDir, query, kind, limit });
}

export async function indexFileSymbols(projectDir: string, filePath: string) {
  return invoke("index_file_symbols", { projectDir, filePath });
}

export async function indexGetContext(projectDir: string, query: string, maxChunks?: number) {
  return invoke("index_get_context", { projectDir, query, maxChunks });
}

export async function indexRepoGetContext(query: string, maxChunks?: number) {
  return invoke("index_repo_get_context", { query, maxChunks });
}

export async function indexSearchInProject(
  projectDir: string,
  query: string,
  caseSensitive: boolean,
  filePattern?: string,
  maxResults?: number,
) {
  return invoke("search_in_project", { projectDir, query, caseSensitive, filePattern, maxResults });
}

// --- AI / config helpers ---

export interface AppSettings {
  storage?: {
    indexPathPolicy?: string;
    allowProjectWrites?: boolean;
  };
  resources?: {
    librarySearchPaths?: string[];
    packageCacheDir?: string;
  };
  documentation?: {
    helpBaseUrl?: string;
    showWelcomeOnFirstLaunch?: boolean;
  };
  extensions?: {
    pluginDir?: string;
    modelicaStdlibPath?: string;
  };
}

export async function getAppSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_app_settings");
}

export async function setAppSettings(settings: AppSettings): Promise<void> {
  await invoke("set_app_settings", { settings });
}

export async function getAppDataRoot(): Promise<string> {
  return invoke<string>("get_app_data_root");
}

export async function getApiKey(): Promise<string> {
  return invoke<string>("get_api_key");
}

export async function setApiKey(apiKey: string): Promise<void> {
  await invoke("set_api_key", { apiKey });
}

export async function aiCodeGen(payload: unknown): Promise<string> {
  return invoke<string>("ai_code_gen", { payload });
}

export async function aiGenerateCompilerPatch(target: string): Promise<string> {
  return invoke<string>("ai_generate_compiler_patch", { target });
}

export async function aiGenerateCompilerPatchWithContext(
  target: string,
  contextFiles: string[],
  testCases: string[],
): Promise<string> {
  return invoke<string>("ai_generate_compiler_patch_with_context", {
    target,
    contextFiles,
    testCases,
  });
}

export interface MoRunDetail {
  name: string;
  expected: string;
  actual: string;
}

export interface MoRunResult {
  passed: number;
  failed: number;
  details: MoRunDetail[];
}

export interface IterationRunResult {
  success: boolean;
  build_ok: boolean;
  test_ok: boolean;
  message: string;
  mo_run?: MoRunResult | null;
  quick_run?: boolean;
}

export interface IterationRecord {
  id: number;
  target: string;
  diff: string | null;
  success: boolean;
  message: string;
  created_at: string;
  branch_name?: string | null;
  duration_ms?: number | null;
  git_commit?: string | null;
}

export async function runSelfIterate(
  diff?: string,
  quick = true,
): Promise<IterationRunResult> {
  return invoke<IterationRunResult>("self_iterate", { diff, quick });
}

export async function applyPatchToWorkspace(diff: string): Promise<void> {
  await invoke("apply_patch_to_workspace", { diff });
}

export async function commitIterationPatch(message: string): Promise<void> {
  await invoke("commit_patch", { message });
}

export async function listIterationHistory(limit = 50): Promise<IterationRecord[]> {
  return invoke<IterationRecord[]>("list_iteration_history", { limit });
}

export async function getIteration(id: number): Promise<IterationRecord | null> {
  return invoke<IterationRecord | null>("get_iteration", { id });
}

export async function saveIteration(
  target: string,
  diff: string | null,
  success: boolean,
  message: string,
  gitCommit?: string | null,
): Promise<number> {
  return invoke<number>("save_iteration", {
    target,
    diff,
    success,
    message,
    git_commit: gitCommit ?? null,
  });
}

export async function gitHeadCommit(projectDir: string): Promise<string> {
  return invoke<string>("git_head_commit", { projectDir });
}

export interface ModelEquationsAndVars {
  modelName: string;
  variables: {
    name: string;
    typeName: string;
    variability: string;
    startValue: string;
    unit: string;
    description: string;
  }[];
  equations: {
    id: string;
    text: string;
    isWhen: boolean;
  }[];
}

export async function extractEquationsFromSource(source: string): Promise<ModelEquationsAndVars> {
  return invoke<ModelEquationsAndVars>("extract_equations_from_source", { source });
}

export async function applyEquationEdits(
  source: string,
  variables: ModelEquationsAndVars["variables"],
  equations: ModelEquationsAndVars["equations"],
): Promise<{ newSource: string }> {
  return invoke<{ newSource: string }>("apply_equation_edits", { source, variables, equations });
}

export interface StepState {
  time: number;
  states: number[];
  stateNames: string[];
  discreteVals: number[];
  outputs: number[];
  outputNames: string[];
  activeEvents: string[];
  stepIndex: number;
}

export async function startSimulationSession(
  code: string,
  modelName?: string,
  projectDir?: string | null,
): Promise<string> {
  return invoke<string>("start_simulation_session", {
    code,
    modelName: modelName ?? undefined,
    projectDir: projectDir ?? undefined,
  });
}

export async function simulationStep(sessionId: string): Promise<StepState> {
  return invoke<StepState>("simulation_step", { sessionId });
}

export async function simulationCommand(
  sessionId: string,
  command: "run" | "pause" | "stop",
): Promise<void> {
  return invoke<void>("simulation_command", { sessionId, command });
}

export async function getSimulationState(sessionId: string): Promise<StepState | null> {
  return invoke<StepState | null>("get_simulation_state", { sessionId });
}

