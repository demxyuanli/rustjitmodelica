function MixedFourScalarOutputs
  input Real x;
  output Real y1;
  output Real y2;
  output Real y3;
  output Real y4;
algorithm
  y1 := x + 1.0;
  y2 := x * 2.0;
  y3 := (x + 1.0) + (x * 2.0);
  y4 := ((x + 1.0) + (x * 2.0)) - x;
end MixedFourScalarOutputs;
