model AlgTest
  Real x(start=0.0);
  Real y;
algorithm
  y := x + 1.0;
  if x > 2.0 then
    y := x + 2.0;
  end if;
equation
  der(x) = 1.0;
end AlgTest;
