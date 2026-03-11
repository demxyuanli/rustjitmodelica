model SmallFor
  Real x[3](each start = 1);
equation
  for i in 1:3 loop
    der(x[i]) = -x[i];
  end for;
end SmallFor;
