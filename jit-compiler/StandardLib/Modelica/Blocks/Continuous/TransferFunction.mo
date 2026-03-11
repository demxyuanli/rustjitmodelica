block TransferFunction
  extends Modelica.Blocks.Interfaces.SISO;
  parameter Real b = 1.0;
  parameter Real a = 1.0;
  Real x(start=0.0);
equation
  der(x) = u.signal - a * x;
  y.signal = b * x;
end TransferFunction;
