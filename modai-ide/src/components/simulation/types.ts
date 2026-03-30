export interface TestAllResultItem {
  path: string;
  success: boolean;
  errors: string[];
}

export interface SimulationChartSeries {
  name: string;
  values: number[];
}

export interface SimulationChartMeta {
  pointCount: number;
  seriesCount: number;
  xMin: number | null;
  xMax: number | null;
}
