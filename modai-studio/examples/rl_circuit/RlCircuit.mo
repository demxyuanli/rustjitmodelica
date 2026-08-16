model RlCircuit
  Modelica.Electrical.Analog.Sources.PulseVoltage vpulse(V = 10, width = 50, period = 0.02, nperiod = -1, offset = 0, startTime = 0);
  Modelica.Electrical.Analog.Sensors.CurrentSensor isen;
  Modelica.Electrical.Analog.Basic.Resistor resistor1(R = 10);
  Modelica.Electrical.Analog.Basic.Inductor inductor1(L = 0.01);
  Modelica.Electrical.Analog.Basic.Ground ground1;
equation
  connect(vpulse.p, isen.p);
  connect(isen.n, resistor1.p);
  connect(resistor1.n, inductor1.p);
  connect(inductor1.n, ground1.p);
  connect(vpulse.n, ground1.p);
  annotation(experiment(StopTime = 0.05, Interval = 0.0001));
end RlCircuit;
