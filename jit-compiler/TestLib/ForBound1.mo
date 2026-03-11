model ForBound1
  Real x(start = 0);
equation
  for i in 1:1 loop
    der(x) = -x;
  end for;
end ForBound1;
