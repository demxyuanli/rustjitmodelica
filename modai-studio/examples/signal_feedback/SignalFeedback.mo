model SignalFeedback
  Modelica.Blocks.Sources.Step step1(height = 1, startTime = 0.5);
  Modelica.Blocks.Math.Gain gain1(k = 2);
  Modelica.Blocks.Continuous.Integrator integrator1(k = 1);
equation
  connect(step1.y, gain1.u);
  connect(gain1.y, integrator1.u);
  annotation(experiment(StopTime = 5, Interval = 0.01));
end SignalFeedback;
