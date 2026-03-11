model Base
  parameter Real k = 1.0;
  Real x(start=10.0);
equation
  der(x) = -k * x;
end Base;
