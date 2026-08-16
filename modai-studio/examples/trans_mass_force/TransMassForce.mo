model TransMassForce
  Modelica.Blocks.Sources.Step step1(height = 1, startTime = 0.2);
  Modelica.Mechanics.Translational.Sources.Force force1;
  Modelica.Mechanics.Translational.Components.Mass mass1(m = 1);
  Modelica.Mechanics.Translational.Components.Damper damper1(d = 0.8);
  Modelica.Mechanics.Translational.Components.Fixed fixed1(s0 = 0);
equation
  connect(step1.y, force1.f);
  connect(force1.flange, mass1.flange_a);
  connect(mass1.flange_b, damper1.flange_a);
  connect(damper1.flange_b, fixed1.flange);
  annotation(experiment(StopTime = 3, Interval = 0.006));
end TransMassForce;
