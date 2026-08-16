model TfPlant
  Modelica.Blocks.Sources.Step step1(height = 1, startTime = 0.2);
  Modelica.Blocks.Continuous.TransferFunction tf1(b = {1}, a = {1, 1});
equation
  connect(step1.y, tf1.u);
  annotation(experiment(StopTime = 5, Interval = 0.01));
end TfPlant;
