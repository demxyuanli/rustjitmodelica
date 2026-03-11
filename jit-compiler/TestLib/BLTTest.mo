model BLTTest
  Real x;
  Real y;
  Real z;
  parameter Real p = 2.0;
equation
  x + y = 10.0;
  x - y = 2.0;
  z = x * p;
end BLTTest;
