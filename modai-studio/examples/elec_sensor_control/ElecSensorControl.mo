model ElecSensorControl
  Modelica.Electrical.Analog.Sources.SineVoltage vsine(V = 10, f = 50, phase = 0, offset = 0, startTime = 0);
  Modelica.Electrical.Analog.Basic.Resistor resistor1(R = 100);
  Modelica.Electrical.Analog.Basic.Ground ground1;
  Modelica.Electrical.Analog.Sensors.VoltageSensor vSense;
  Modelica.Blocks.Math.Gain gain1(k = 0.1);
  Modelica.Blocks.Routing.RealPassThrough display1;
equation
  connect(vsine.p, resistor1.p);
  connect(resistor1.n, ground1.p);
  connect(vsine.n, ground1.p);
  connect(resistor1.p, vSense.p);
  connect(resistor1.n, vSense.n);
  connect(vSense.v, gain1.u);
  connect(gain1.y, display1.u);
  annotation(experiment(StopTime = 0.05, Interval = 0.0001));
end ElecSensorControl;
