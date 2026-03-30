import { invoke } from "@tauri-apps/api/core";


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

