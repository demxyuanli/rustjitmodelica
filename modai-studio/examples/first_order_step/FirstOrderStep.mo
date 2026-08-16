model FirstOrderStep
  Modelica.Blocks.Sources.Step step1(height = 1, startTime = 0.5);
  Modelica.Blocks.Continuous.FirstOrder plant(k = 1, T = 0.4);
equation
  connect(step1.y, plant.u);
  annotation(experiment(StopTime = 5, Interval = 0.01));
end FirstOrderStep;
