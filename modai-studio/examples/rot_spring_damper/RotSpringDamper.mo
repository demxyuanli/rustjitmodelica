model RotSpringDamper
  Modelica.Blocks.Sources.Step step1(height = 1, startTime = 0.2);
  Modelica.Mechanics.Rotational.Sources.Torque torque1;
  Modelica.Mechanics.Rotational.Components.Inertia inertia1(J = 1);
  Modelica.Mechanics.Rotational.Components.SpringDamper springDamper1(c = 10, d = 0.5, phi_rel0 = 0);
  Modelica.Mechanics.Rotational.Components.Fixed fixed1(phi0 = 0);
  Modelica.Mechanics.Rotational.Sensors.SpeedSensor wSensor;
  Modelica.Blocks.Routing.RealPassThrough scope1;
equation
  connect(step1.y, torque1.tau);
  connect(torque1.flange, inertia1.flange_a);
  connect(inertia1.flange_b, springDamper1.flange_a);
  connect(springDamper1.flange_b, fixed1.flange);
  connect(inertia1.flange_a, wSensor.flange);
  connect(wSensor.w, scope1.u);
  annotation(experiment(StopTime = 2, Interval = 0.005));
end RotSpringDamper;
