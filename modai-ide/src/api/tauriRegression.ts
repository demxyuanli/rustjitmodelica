import { invoke } from "@tauri-apps/api/core";
import type { RegressionPlanRequest, RegressionWorkspaceInfo, RegressionWorkspaceState } from "../types";

export interface LibraryRegressionOptions {
  includeModelicaExamples?: boolean;
  includeModelicaTest?: boolean;
  maxCases?: number;
  solver?: string;
  tEnd?: number;
  dt?: number;
  extraArgs?: string[];
}

export interface PlannedRegressionCase {
  name: string;
  reason: string;
  priority: number;
}

export interface RegressionExecutionPlan {
  changedSources: string[];
  affectedFeatures: string[];
  plannedCases: PlannedRegressionCase[];
  skippedCases: string[];
}

export async function traceabilityBuildExecutionPlan(): Promise<RegressionExecutionPlan> {
  return invoke<RegressionExecutionPlan>("traceability_build_execution_plan");
}

export async function runLibraryRegression(
  options?: LibraryRegressionOptions,
): Promise<{
  total: number;
  passed: number;
  failed: number;
  results: Array<{
    name: string;
    passed: boolean;
    exitCode: number;
    stdout: string;
    stderr: string;
    durationMs: number;
    failureKind?: string | null;
    retries: number;
  }>;
  durationMs: number;
}> {
  return invoke("run_library_regression", { options });
}

export async function regressionCreateWorkspace(
  request: RegressionPlanRequest,
): Promise<RegressionWorkspaceState> {
  return invoke<RegressionWorkspaceState>("regression_create_workspace", { request });
}

export async function regressionRunWorkspace(workspaceId: string): Promise<RegressionWorkspaceState> {
  return invoke<RegressionWorkspaceState>("regression_run_workspace", { workspaceId });
}

export async function regressionGetWorkspaceState(workspaceId: string): Promise<RegressionWorkspaceState> {
  return invoke<RegressionWorkspaceState>("regression_get_workspace_state", { workspaceId });
}

export async function regressionListWorkspaces(): Promise<RegressionWorkspaceInfo[]> {
  return invoke<RegressionWorkspaceInfo[]>("regression_list_workspaces");
}

export async function regressionCancelWorkspace(workspaceId: string): Promise<RegressionWorkspaceState> {
  return invoke<RegressionWorkspaceState>("regression_cancel_workspace", { workspaceId });
}
