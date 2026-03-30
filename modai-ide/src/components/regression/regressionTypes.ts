import type { RegressionRunRecord } from "../../types";

export type NormalizedRegressionRecord = RegressionRunRecord & { parsedCategory: string };
