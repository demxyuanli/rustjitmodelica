model TearingTest
  Real x(start=1.0);
  Real y(start=2.0);
  Real z(start=3.0);
equation
  der(x) = -x + y;
  
  // Algebraic Loop: y depends on z, z depends on y
  y = 2 * z + x;
  z = 3 * y - 5;
end TearingTest;
