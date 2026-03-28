model MultiAssignComprehension
  Real x(start = 2.0);
  Real a;
  Real b;
  Real c;
equation
  der(x) = 0.0;
algorithm
  (a, b, c) := TestLib.ComprehensionAndScalarOutputs(x);
end MultiAssignComprehension;
