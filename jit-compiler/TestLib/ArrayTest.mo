model ArrayTest
  Real x[3];
  parameter Real p[3] = {10, 20, 30};
  Real y;
equation
  x = p;
  y = x[1] + p[2];
end ArrayTest;
