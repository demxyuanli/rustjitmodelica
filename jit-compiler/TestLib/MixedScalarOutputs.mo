function MixedScalarOutputs
  input Real x;
  output Real y1;
  output Real y2;
  output Real y3;
protected
  Real tmp[2];
algorithm
  tmp[1] := x + 1.0;
  tmp[2] := x * 2.0;
  y1 := tmp[1];
  y2 := tmp[2];
  y3 := y1 + y2;
end MixedScalarOutputs;
