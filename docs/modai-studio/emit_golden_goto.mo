model GateGoto
  Modelica.Blocks.Sources.Step step1(height = 1);
  Modelica.Blocks.Routing.RealPassThrough goto1;
  Modelica.Blocks.Routing.RealPassThrough from1;
  Modelica.Blocks.Math.Gain gain1(k = 1);
equation
  connect(step1.y, goto1.u);
  connect(from1.y, gain1.u);
  connect(goto1.y, from1.u);
end GateGoto;
