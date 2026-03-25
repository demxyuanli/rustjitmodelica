model MixedNestedLhsFieldStoreMismatch
  Real x(start = 0.0);
  Point pts[2];
  Real b;
equation
  der(x) = 1.0;
algorithm
  (pts[1].x, b) := TestLib.MixedScalarOutputs(x);
end MixedNestedLhsFieldStoreMismatch;
