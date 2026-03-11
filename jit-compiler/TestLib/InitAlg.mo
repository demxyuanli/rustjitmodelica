model InitAlg
  parameter Real a = 1;
  parameter Real b = 4;
  Real x(start = 0);
  Real y(start = 0);
initial algorithm
  x := a + b;
  y := (a + b) * 2;
equation
  der(x) = -x;
  der(y) = -y;
end InitAlg;

