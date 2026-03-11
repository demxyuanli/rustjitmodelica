model LibraryTest
  Modelica.Blocks.Sources.Sine sine(freqHz=1.0, amplitude=2.0);
  Modelica.Blocks.Continuous.Integrator integrator(k=1.0);
  Real y_min;
  Real y_max;
  Real y_mod;
  Real y_sign;
equation
  connect(sine.y, integrator.u);
  
  y_min = Modelica.Math.min(sine.y.signal, integrator.y.signal);
  y_max = Modelica.Math.max(sine.y.signal, integrator.y.signal);
  y_mod = Modelica.Math.mod(time, 2.0);
  y_sign = Modelica.Math.sign(sine.y.signal);
end LibraryTest;
