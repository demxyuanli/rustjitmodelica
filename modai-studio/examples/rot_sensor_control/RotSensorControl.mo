model RotSensorControl
  Modelica.Blocks.Sources.Step step1(height = 1, startTime = 0.2);
  Modelica.Mechanics.Rotational.Sources.Torque torque1;
  Modelica.Mechanics.Rotational.Components.Inertia inertia1(J = 1);
  Modelica.Mechanics.Rotational.Components.Damper damper1(d = 0.5);
  Modelica.Mechanics.Rotational.Components.Fixed fixed1(phi0 = 0);
  Modelica.Mechanics.Rotational.Sensors.SpeedSensor wSensor;
  Modelica.Mechanics.Rotational.Sensors.AngleSensor angleSensor;
  Modelica.Blocks.Routing.RealPassThrough scope1;
equation
  connect(step1.y, torque1.tau);
  connect(torque1.flange, inertia1.flange_a);
  connect(inertia1.flange_b, damper1.flange_a);
  connect(damper1.flange_b, fixed1.flange);
  connect(inertia1.flange_a, wSensor.flange);
  connect(inertia1.flange_b, angleSensor.flange);
  connect(wSensor.w, scope1.u);
  annotation(experiment(StopTime = 2, Interval = 0.005));
end RotSensorControl;
