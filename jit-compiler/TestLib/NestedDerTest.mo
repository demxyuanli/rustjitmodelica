model NestedDerTest
  Real x(start = 0);
  Real y;
equation
  der(x) = 1;
  y = der(x) + 2;
end NestedDerTest;
