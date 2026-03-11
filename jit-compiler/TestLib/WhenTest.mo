model WhenTest
  Real x(start=0.0);
  Real y;
algorithm
  y := 0.0;
  when x > 2.5 then
    y := 10.0;
  end when;
equation
  der(x) = 1.0;
end WhenTest;
