model LoopTest
  parameter Integer N = 100;
  Real x[N](start=1.0);
  Real y[N](start=0.0);
  Real sum_x;
equation
  for i in 1:N loop
    der(x[i]) = -y[i];
    der(y[i]) = x[i];
  end for;
  
  sum_x = 0;
end LoopTest;
