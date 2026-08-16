model SwitchSelect
  Modelica.Blocks.Sources.Constant hi(k = 1);
  Modelica.Blocks.Sources.Constant lo(k = 0);
  Modelica.Blocks.Sources.BooleanStep sw(startTime = 0.5, startValue = false);
  Modelica.Blocks.Logical.Switch switch1;
equation
  connect(hi.y, switch1.u1);
  connect(sw.y, switch1.u2);
  connect(lo.y, switch1.u3);
  annotation(experiment(StopTime = 2, Interval = 0.005));
end SwitchSelect;
