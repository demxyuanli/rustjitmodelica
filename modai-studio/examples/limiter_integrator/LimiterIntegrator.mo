model LimiterIntegrator
  Modelica.Blocks.Sources.Step step1(height = 2, startTime = 0.2);
  Modelica.Blocks.Nonlinear.Limiter limiter1(uMax = 1, uMin = -1);
  Modelica.Blocks.Continuous.Integrator integrator1(k = 1);
equation
  connect(step1.y, limiter1.u);
  connect(limiter1.y, integrator1.u);
  annotation(experiment(StopTime = 4, Interval = 0.01));
end LimiterIntegrator;
