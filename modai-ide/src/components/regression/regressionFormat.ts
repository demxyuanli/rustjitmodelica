import { t } from "../../i18n";
import type { RecordPreset } from "./regressionConstants";

export function pct(a: number, b: number): string {
  if (!b) return "0.0%";
  return `${((a / b) * 100).toFixed(1)}%`;
}

export function statusTone(status: string): string {
  if (status === "completed") return "theme-banner-success";
  if (status === "failed" || status === "cancelled") return "theme-banner-danger";
  if (status === "running") return "theme-banner-warning";
  return "theme-button-secondary";
}

export function parseCategoryFromDetail(detail: string): string {
  const marker = "category=";
  const idx = detail.indexOf(marker);
  if (idx < 0) return "unknown";
  const raw = detail.slice(idx + marker.length).trim();
  const endByPipe = raw.indexOf("|");
  const endByComma = raw.indexOf(",");
  const endBySpace = raw.indexOf(" ");
  const ends = [endByPipe, endByComma, endBySpace].filter((x) => x >= 0);
  const end = ends.length > 0 ? Math.min(...ends) : raw.length;
  const value = raw.slice(0, end).trim();
  return value || "unknown";
}

export function workspaceStatusLabel(status: string): string {
  const map: Record<string, string> = {
    planned: t("regressionStatusPlanned"),
    running: t("regressionStatusRunning"),
    completed: t("regressionStatusCompleted"),
    failed: t("regressionStatusFailed"),
    cancelled: t("regressionStatusCancelled"),
  };
  return map[status] ?? status;
}

export function reasonLabel(reason: string): string {
  const map: Record<string, string> = {
    expectationMet: t("regressionReasonExpectationMet"),
    modelNotFound: t("regressionReasonModelNotFound"),
    dependencyMissing: t("regressionReasonDependencyMissing"),
    newtonNonconverged: t("regressionReasonNewtonNonconverged"),
    parseError: t("regressionReasonParseError"),
    runtimeError: t("regressionReasonRuntimeError"),
    timeout: t("regressionReasonTimeout"),
    processError: t("regressionReasonProcessError"),
    cancelled: t("regressionReasonCancelled"),
  };
  return map[reason] ?? reason;
}

export function presetLabel(preset: RecordPreset): string {
  if (preset === "failed-triage") return t("regressionPresetFailedTriage");
  if (preset === "slowest") return t("regressionPresetSlowest");
  return t("regressionPresetLatest");
}

export function recordKeyOf(r: { timestamp: string; caseName: string }): string {
  return `${r.timestamp}-${r.caseName}`;
}
