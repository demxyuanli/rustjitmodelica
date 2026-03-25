model MixedNestedTargetsPass
  Real x(start = 0.0);
  Real arr[2];
  Real b;
equation
  der(x) = 1.0;
  (arr[1], b) = TestLib.TwoOutputs(x);
  arr[2] = arr[1] + b;
end MixedNestedTargetsPass;
