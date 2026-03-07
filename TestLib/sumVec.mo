function sumVec
  input Real v[:];
  output Real s;
algorithm
  s := 0.0;
  for i in 1:size(v, 1) loop
    s := s + v[i];
  end for;
end sumVec;
