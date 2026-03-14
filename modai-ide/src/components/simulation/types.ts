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
