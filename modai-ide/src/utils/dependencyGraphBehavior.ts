import type { AppSettings, EquationGraphMode } from "../api/tauri";

export type DependencyGraphBehavior = {
  fullTimeoutSec: number;
  autoDowngradeFromFull: boolean;
  downgradeTarget: "compact" | "top-level";
  initialGraphMode: EquationGraphMode;
};

const DEFAULT_BEHAVIOR: DependencyGraphBehavior = {
  fullTimeoutSec: 8,
  autoDowngradeFromFull: true,
  downgradeTarget: "compact",
  initialGraphMode: "compact",
};

function normalizeEquationGraphMode(raw: string | undefined): EquationGraphMode {
  const v = (raw ?? "compact").trim().toLowerCase().replace(/_/g, "-");
  if (v === "structural") return "structural";
  if (v === "full") return "full";
  if (v === "top-level" || v === "toplevel") return "top-level";
  return "compact";
}

export function dependencyGraphBehaviorFromAppSettings(
  appSettings: AppSettings | null | undefined
): DependencyGraphBehavior {
  const dg = appSettings?.dependencyGraph;
  const raw = dg?.fullTimeoutSec ?? DEFAULT_BEHAVIOR.fullTimeoutSec;
  const n = typeof raw === "number" && Number.isFinite(raw) ? Math.floor(raw) : DEFAULT_BEHAVIOR.fullTimeoutSec;
  const fullTimeoutSec = Math.min(300, Math.max(1, n));
  const preferStructural = dg?.preferStructuralFirst ?? false;
  const configured = normalizeEquationGraphMode(dg?.defaultGraphMode);
  const initialGraphMode: EquationGraphMode = preferStructural ? "structural" : configured;
  return {
    fullTimeoutSec,
    autoDowngradeFromFull: dg?.autoDowngradeFromFull ?? DEFAULT_BEHAVIOR.autoDowngradeFromFull,
    downgradeTarget: dg?.downgradeTarget === "top-level" ? "top-level" : "compact",
    initialGraphMode,
  };
}
