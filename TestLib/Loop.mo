model Loop
  parameter Integer n = 5;
  Real x[n](start=10.0);
equation
  for i in 1:n loop
    der(x[i]) = -x[i];
  end for;
end Loop;