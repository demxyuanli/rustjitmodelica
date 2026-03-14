model ComponentLibraryCoverage
  Modelica.Blocks.Sources.Step step(height=2.0, startTime=0.5);
  Modelica.Blocks.Sources.Sine sine(amplitude=1.5, freqHz=0.5);
  Modelica.Blocks.Continuous.TransferFunction tf(b=2.0, a=1.0);
  Modelica.Blocks.Continuous.TransferFunction tfAux(b=1.0, a=2.0);
  Modelica.Blocks.Continuous.Integrator integrator(k=0.75, y_start=0.25);
  Modelica.Blocks.Continuous.Integrator integratorAux(k=0.5, y_start=0.0);

  VoltageSource src(V=12.0);
  Resistor r1(R=10.0);
  Resistor r2(R=25.0);
  Ground g;
equation
  connect(step.y, tf.u);
  connect(tf.y, integrator.u);
  connect(sine.y, tfAux.u);
  connect(tfAux.y, integratorAux.u);

  connect(src.p, r1.p);
  connect(r1.n, r2.p);
  connect(r2.n, src.n);
  connect(src.n, g.p);
end ComponentLibraryCoverage;
