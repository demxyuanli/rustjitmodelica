model MultiOutputMixedArrayScalar
  Real x(start = 0.0);
  Real arr[2];
  Real s;
equation
  der(x) = 1.0;
  (arr[1], arr[2], s) = TestLib.MixedScalarOutputs(x);
end MultiOutputMixedArrayScalar;
