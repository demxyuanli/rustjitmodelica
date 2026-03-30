import { invoke } from "@tauri-apps/api/core";


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

export interface IndexIncludedFiles {
  total: number;
  paths: string[];
}

export async function indexListIncludedFiles(
  projectDir: string,
  limit?: number
): Promise<IndexIncludedFiles> {
  return invoke<IndexIncludedFiles>("index_list_included_files", {
    projectDir,
    limit,
  });
}

export async function indexRepoStats() {
  return invoke("index_repo_stats");
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

export async function indexComponentLibraryGetContext(query: string, maxChunks?: number) {
  return invoke("index_component_library_get_context", { query, maxChunks });
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

