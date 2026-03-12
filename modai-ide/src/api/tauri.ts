import { invoke } from "@tauri-apps/api/core";
import type { JitValidateOptions, JitValidateResult, SimulationResult } from "../types";

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

// --- Project / Modelica files ---

export async function openProjectDir(): Promise<string | null> {
  return invoke<string | null>("open_project_dir");
}

export async function listMoTree(projectDir: string) {
  return invoke("list_mo_tree", { projectDir });
}

export async function listMoFiles(projectDir: string) {
  return invoke<string[]>("list_mo_files", { projectDir });
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

export async function getApiKey(): Promise<string> {
  return invoke<string>("get_api_key");
}

export async function setApiKey(apiKey: string): Promise<void> {
  await invoke("set_api_key", { apiKey });
}

export async function aiCodeGen(payload: unknown): Promise<string> {
  return invoke<string>("ai_code_gen", { payload });
}

