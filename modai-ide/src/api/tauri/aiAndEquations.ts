import { invoke } from "@tauri-apps/api/core";


// --- AI / config helpers ---

export interface IndexCacheSettings {
  componentLibraryIndexEnabled?: boolean;
  repoIndexRefreshOnJitLoad?: boolean;
  gitStatusThrottleMs?: number;
}

export interface IndexingSettings {
  indexAutoNewFolders?: boolean;
  indexAutoNewFoldersMaxFiles?: number;
  indexRepoForGrep?: boolean;
}

export type AiScope = "user" | "project" | "rustmodlica" | "all";

export interface AiRule {
  id: string;
  name: string;
  scope: AiScope;
  enabled: boolean;
  content: string;
}

export interface AiSkill {
  id: string;
  name: string;
  description: string;
  scope: AiScope;
  enabled: boolean;
  content: string;
}

export interface AiSubagent {
  id: string;
  name: string;
  description: string;
  scope: AiScope;
  enabled: boolean;
  content: string;
}

export interface AiCommand {
  id: string;
  name: string;
  description: string;
  scope: AiScope;
  enabled: boolean;
  content: string;
}

export interface AiConfig {
  rules: AiRule[];
  skills: AiSkill[];
  subagents: AiSubagent[];
  commands: AiCommand[];
  /** Model IDs enabled in the AI panel. If undefined or empty, all built-in models are shown. */
  modelIdsEnabled?: string[] | null;
}

export interface DependencyGraphSettings {
  fullTimeoutSec?: number;
  autoDowngradeFromFull?: boolean;
  /** Serialized as kebab-case: "compact" | "top-level" */
  downgradeTarget?: "compact" | "top-level";
  defaultGraphMode?: string;
  preferStructuralFirst?: boolean;
}

export interface ValidationSettings {
  defaultTier?: string;
  eqExpandParallelMode?: "off" | "guarded" | "on";
}

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
  indexCache?: IndexCacheSettings;
  indexing?: IndexingSettings;
  dependencyGraph?: DependencyGraphSettings;
  validation?: ValidationSettings;
  ai?: AiConfig;
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

export async function rebuildComponentLibraryIndex(): Promise<void> {
  await invoke("rebuild_component_library_index");
}

export async function getApiKey(): Promise<string> {
  return invoke<string>("get_api_key");
}

export async function setApiKey(apiKey: string): Promise<void> {
  await invoke("set_api_key", { apiKey });
}

export async function getGrokApiKey(): Promise<string> {
  return invoke<string>("get_grok_api_key");
}

export async function setGrokApiKey(apiKey: string): Promise<void> {
  await invoke("set_grok_api_key", { apiKey });
}

export interface AiCodeGenResult {
  content: string;
  toolCallsUsed?: string[];
}

export async function aiCodeGen(payload: unknown): Promise<AiCodeGenResult> {
  const raw = await invoke<string>("ai_code_gen", { payload });
  try {
    const obj = JSON.parse(raw) as { content?: string; tool_calls_used?: string[] };
    if (typeof obj?.content === "string") {
      return {
        content: obj.content,
        toolCallsUsed: Array.isArray(obj.tool_calls_used) ? obj.tool_calls_used : undefined,
      };
    }
  } catch {
    // not JSON, treat as plain content
  }
  return { content: raw };
}

export async function aiCodeGenStream(requestId: string, payload: unknown): Promise<void> {
  await invoke("ai_code_gen_stream", { requestId, payload });
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

export async function applyPatchToProject(projectDir: string, diff: string): Promise<void> {
  await invoke("apply_patch_to_project", { projectDir, diff });
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

