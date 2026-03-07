model ArrayLoopTest
  Real x[3](start = {1.0, 2.0, 3.0});
  Real y[3];
equation
  der(x) = -x;
  y[1] = 2.0 * x[1];
  y[2] = 2.0 * x[2];
  y[3] = 2.0 * x[3];
end ArrayLoopTest;
