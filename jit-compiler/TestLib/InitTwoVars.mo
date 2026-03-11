model InitTwoVars
  Real x(start = 0);
  Real y(start = 0);
initial equation
  y = 1.0;
  x = 2.0 * y;
equation
  der(x) = -x;
  der(y) = -y;
end InitTwoVars;
