model GotoFrom
  Modelica.Blocks.Sources.Step step1(height = 1, startTime = 0.2);
  Modelica.Blocks.Routing.RealPassThrough goto1;
  Modelica.Blocks.Routing.RealPassThrough from1;
  Modelica.Blocks.Math.Gain gain1(k = 2);
equation
  connect(step1.y, goto1.u);
  connect(goto1.y, from1.u);
  connect(from1.y, gain1.u);
  annotation(experiment(StopTime = 2, Interval = 0.01));
end GotoFrom;
