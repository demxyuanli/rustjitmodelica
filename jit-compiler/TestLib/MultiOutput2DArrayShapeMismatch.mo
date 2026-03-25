model MultiOutput2DArrayShapeMismatch
  Real x(start = 0.0);
  Real a;
  Real b;
equation
  der(x) = 1.0;
algorithm
  (a, b) := TestLib.MatrixAndScalarOutputs(x);
end MultiOutput2DArrayShapeMismatch;
