model TransSpringDamper
  Modelica.Blocks.Sources.Step step1(height = 1, startTime = 0.2);
  Modelica.Mechanics.Translational.Sources.Force force1;
  Modelica.Mechanics.Translational.Components.Mass mass1(m = 1);
  Modelica.Mechanics.Translational.Components.SpringDamper springDamper1(c = 10, d = 0.5, s_rel0 = 0);
  Modelica.Mechanics.Translational.Components.Fixed fixed1(s0 = 0);
  Modelica.Mechanics.Translational.Sensors.SpeedSensor vSensor;
  Modelica.Blocks.Routing.RealPassThrough scope1;
equation
  connect(step1.y, force1.f);
  connect(force1.flange, mass1.flange_a);
  connect(mass1.flange_b, springDamper1.flange_a);
  connect(springDamper1.flange_b, fixed1.flange);
  connect(mass1.flange_a, vSensor.flange);
  connect(vSensor.v, scope1.u);
  annotation(experiment(StopTime = 2, Interval = 0.005));
end TransSpringDamper;
