model RcCircuit
  Modelica.Electrical.Analog.Sources.StepVoltage vstep(V = 10, startTime = 0.1);
  Modelica.Electrical.Analog.Basic.Resistor resistor1(R = 100);
  Modelica.Electrical.Analog.Basic.Capacitor capacitor1(C = 1e-3);
  Modelica.Electrical.Analog.Basic.Ground ground1;
equation
  connect(vstep.p, resistor1.p);
  connect(resistor1.n, capacitor1.p);
  connect(capacitor1.n, ground1.p);
  connect(vstep.n, ground1.p);
  annotation(experiment(StopTime = 1, Interval = 0.002));
end RcCircuit;
