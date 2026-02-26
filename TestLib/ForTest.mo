model ForTest
  Real s;
algorithm
  s := 0.0;
  for i in 1.0:5.0 loop
    s := s + i;
  end for;
end ForTest;
