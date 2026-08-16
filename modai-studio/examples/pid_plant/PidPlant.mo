model PidPlant
  Modelica.Blocks.Sources.Step step1(height = 1, startTime = 0.5);
  Modelica.Blocks.Math.Feedback feedback1;
  Modelica.Blocks.Continuous.PID pid1(k = 2, Ti = 0.5, Td = 0.05);
  Modelica.Blocks.Continuous.FirstOrder plant(k = 1, T = 0.8);
equation
  connect(step1.y, feedback1.u1);
  connect(feedback1.y, pid1.u);
  connect(pid1.y, plant.u);
  connect(plant.y, feedback1.u2);
  annotation(experiment(StopTime = 8, Interval = 0.01));
end PidPlant;
