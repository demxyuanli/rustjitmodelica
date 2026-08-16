model RotDrive
  Modelica.Blocks.Sources.Step step1(height = 1, startTime = 0.2);
  Modelica.Mechanics.Rotational.Sources.Torque torque1;
  Modelica.Mechanics.Rotational.Components.Inertia inertia1(J = 1);
  Modelica.Mechanics.Rotational.Components.Damper damper1(d = 0.5);
  Modelica.Mechanics.Rotational.Components.Fixed fixed1(phi0 = 0);
equation
  connect(step1.y, torque1.tau);
  connect(torque1.flange, inertia1.flange_a);
  connect(inertia1.flange_b, damper1.flange_a);
  connect(damper1.flange_b, fixed1.flange);
  annotation(experiment(StopTime = 3, Interval = 0.006));
end RotDrive;
