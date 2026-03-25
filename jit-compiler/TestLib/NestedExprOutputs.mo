function NestedExprOutputs
  input Real x;
  output Real y1;
  output Real y2;
algorithm
  y1 := (x + 1.0) * (x - 1.0);
  y2 := if x > 0.0 then (x * x + 2.0) else (x - 2.0);
end NestedExprOutputs;
