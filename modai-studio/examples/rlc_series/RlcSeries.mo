model RlcSeries
  Modelica.Electrical.Analog.Sources.ConstantVoltage vsrc(V = 10);
  Modelica.Electrical.Analog.Basic.Resistor resistor1(R = 10);
  Modelica.Electrical.Analog.Basic.Inductor inductor1(L = 0.01);
  Modelica.Electrical.Analog.Basic.Capacitor capacitor1(C = 1e-4);
  Modelica.Electrical.Analog.Sensors.VoltageSensor vsens;
  Modelica.Electrical.Analog.Basic.Ground ground1;
equation
  connect(vsrc.p, resistor1.p);
  connect(resistor1.n, inductor1.p);
  connect(inductor1.n, capacitor1.p);
  connect(capacitor1.n, ground1.p);
  connect(vsrc.n, ground1.p);
  connect(capacitor1.p, vsens.p);
  connect(capacitor1.n, vsens.n);
  annotation(experiment(StopTime = 0.05, Interval = 0.0001));
end RlcSeries;
