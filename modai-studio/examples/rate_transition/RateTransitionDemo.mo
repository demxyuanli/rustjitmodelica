model RateTransitionDemo
  Modelica.Blocks.Sources.Sine sine1(amplitude = 1, f = 1.28, startTime = 0.17);
  Modelica.Blocks.Discrete.ZeroOrderHold rt1(samplePeriod = 0.38, startTime = 0);
  Modelica.Blocks.Math.Gain gain1(k = 1);
  Modelica.Blocks.Discrete.UnitDelay ud1(samplePeriod = 0.1, startTime = 0.19, y_start = 0);
equation
  connect(sine1.y, rt1.u);
  connect(rt1.y, gain1.u);
  connect(gain1.y, ud1.u);
  annotation(experiment(StopTime = 2, Interval = 0.005));
end RateTransitionDemo;
