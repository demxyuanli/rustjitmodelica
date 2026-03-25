model MixedMultiTargetSafePass
  Real x(start = 0.0);
  Real arr[2];
  Real b;
  Real c;
equation
  der(x) = 1.0;
algorithm
  (arr[1], b, arr[2], c) := TestLib.MixedFourScalarOutputs(x);
end MixedMultiTargetSafePass;
