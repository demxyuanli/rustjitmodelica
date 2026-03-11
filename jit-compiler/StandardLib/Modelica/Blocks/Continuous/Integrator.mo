block Integrator
  extends Modelica.Blocks.Interfaces.SISO;
  parameter Real k = 1.0;
  parameter Real y_start = 0.0;
  Real x(start=y_start);
equation
  der(x) = k * u.signal;
  y.signal = x;
end Integrator;
