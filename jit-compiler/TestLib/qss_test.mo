model qss_test
  Real x(start=1.0);
  Real y(start=0.0);
equation
  der(x) = -0.5 * x;
  der(y) = x - 0.2 * y;
end qss_test;
