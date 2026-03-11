model WhileTest
  Real x(start=0.0);
  Real y;
algorithm
  y := 0.0;
  while x < 5.0 loop
    x := x + 1.0;
    y := y + 2.0;
  end while;
equation
end WhileTest;
