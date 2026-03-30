export const CATEGORY_OPTIONS = [
  "basic",
  "initialization",
  "array",
  "connect",
  "discrete",
  "algebraic",
  "solver",
  "function",
  "structure",
  "msl",
  "tooling",
  "error",
];

export type RecordPreset = "latest" | "failed-triage" | "slowest";

export type BatchField =
  | "caseName"
  | "status"
  | "reason"
  | "category"
  | "exitCode"
  | "durationMs"
  | "timestamp";

export type CliCaseMode = "combined" | "repeated";

export const BATCH_FIELD_OPTIONS: Array<{
  key: BatchField;
  labelKey:
    | "name"
    | "status"
    | "regressionFilterReason"
    | "regressionFilterCategory"
    | "exitLabel"
    | "durationLabel"
    | "regressionSortTime";
}> = [
  { key: "caseName", labelKey: "name" },
  { key: "status", labelKey: "status" },
  { key: "reason", labelKey: "regressionFilterReason" },
  { key: "category", labelKey: "regressionFilterCategory" },
  { key: "exitCode", labelKey: "exitLabel" },
  { key: "durationMs", labelKey: "durationLabel" },
  { key: "timestamp", labelKey: "regressionSortTime" },
];
