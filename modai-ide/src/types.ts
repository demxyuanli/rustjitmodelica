export interface JitValidateOptions {
  t_end?: number;
  dt?: number;
  atol?: number;
  rtol?: number;
  solver?: string;
  output_interval?: number;
}

export interface WarningItem {
  path: string;
  line: number;
  column: number;
  message: string;
}

export interface JitValidateResult {
  success: boolean;
  warnings: WarningItem[];
  errors: string[];
  state_vars: string[];
  output_vars: string[];
}

export interface SimulationResult {
  time: number[];
  series: Record<string, number[]>;
}
