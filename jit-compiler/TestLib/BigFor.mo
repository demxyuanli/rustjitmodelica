model BigFor
  Real x[150](each start = 0);
equation
  for i in 1:150 loop
    der(x[i]) = -x[i];
  end for;
end BigFor;
