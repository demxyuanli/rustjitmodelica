model MixedMultiTargetFieldStoreFail
  Real x(start = 0.0);
  Real arr[2];
  Point pts[2];
  Real b;
equation
  der(x) = 1.0;
algorithm
  (arr[1], pts[1].x, arr[2], b) := TestLib.MixedFourScalarOutputs(x);
end MixedMultiTargetFieldStoreFail;
