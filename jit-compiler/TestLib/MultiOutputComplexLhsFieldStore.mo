model MultiOutputComplexLhsFieldStore
  Real x(start = 0.0);
  Point pts[2];
equation
  der(x) = 1.0;
algorithm
  (pts[1].x, pts[2].y) := TestLib.MixedScalarOutputs(x);
end MultiOutputComplexLhsFieldStore;
