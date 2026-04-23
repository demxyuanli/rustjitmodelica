import { invoke } from "@tauri-apps/api/core";
import type {
  EquationGraph,
  JitApiEnvelope,
  JitValidateOptions,
  JitValidateResult,
  SimulationResult,
} from "../../types";

export type EquationGraphMode = "full" | "compact" | "top-level" | "structural";
export type EquationGraphNodeKey =
  | { Equation: { index: number; hash: number } }
  | { Variable: number }
  | { TopLevelComponent: number };

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

export async function jitValidateV2(
  request: JitValidateRequest
): Promise<JitApiEnvelope<JitValidateResult>> {
  return invoke<JitApiEnvelope<JitValidateResult>>("jit_validate_v2", { request });
}

export async function runSimulation(request: RunSimulationRequest): Promise<SimulationResult> {
  return invoke<SimulationResult>("run_simulation_cmd", { request });
}

export async function runSimulationV2(
  request: RunSimulationRequest
): Promise<JitApiEnvelope<SimulationResult>> {
  return invoke<JitApiEnvelope<SimulationResult>>("run_simulation_cmd_v2", { request });
}

export async function getEquationGraph(
  code: string,
  modelName: string,
  projectDir?: string | null,
  graphMode: EquationGraphMode = "compact",
  changedKeys?: EquationGraphNodeKey[] | null,
  metricsSessionId?: string | null,
): Promise<EquationGraph> {
  return invoke<EquationGraph>("get_equation_graph", {
    code,
    modelName,
    projectDir: projectDir ?? undefined,
    graphMode,
    changedKeys: changedKeys ?? undefined,
    metricsSessionId: metricsSessionId ?? undefined,
  });
}

export async function getEquationGraphV2(
  code: string,
  modelName: string,
  projectDir?: string | null,
  graphMode: EquationGraphMode = "compact",
  changedKeys?: EquationGraphNodeKey[] | null,
  metricsSessionId?: string | null,
): Promise<JitApiEnvelope<EquationGraph>> {
  return invoke<JitApiEnvelope<EquationGraph>>("get_equation_graph_v2", {
    code,
    modelName,
    projectDir: projectDir ?? undefined,
    graphMode,
    changedKeys: changedKeys ?? undefined,
    metricsSessionId: metricsSessionId ?? undefined,
  });
}

export interface MonitorEventRecord {
  tsMillis: number;
  category?: "control" | "progress" | "error";
  sessionId?: string;
  task: string;
  stage: string;
  elapsedSec: number;
  message: string;
  currentStep?: number;
  totalSteps?: number;
  reason?: string;
}

export async function getMonitorEvents(
  sessionId?: string | null,
  limit = 200,
): Promise<MonitorEventRecord[]> {
  return invoke<MonitorEventRecord[]>("get_monitor_events", {
    sessionId: sessionId ?? undefined,
    limit,
  });
}

export function formatMonitorReplayLine(r: MonitorEventRecord): string {
  const category = r.category ?? "progress";
  const stepPart =
    r.currentStep != null && r.totalSteps != null ? ` step=${r.currentStep}/${r.totalSteps}` : "";
  const reasonPart = r.reason ? ` reason=${r.reason}` : "";
  return `[${category}] [replay:${r.task}] ${r.message}${stepPart}${reasonPart}`;
}

export interface MonitorEventSessionEntry {
  sessionId: string;
  modifiedMs?: number;
  eventCount: number;
}

export async function listMonitorEventSessions(limit = 50): Promise<MonitorEventSessionEntry[]> {
  return invoke<MonitorEventSessionEntry[]>("list_monitor_event_sessions", { limit });
}

