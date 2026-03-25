model MultiOutputNestedExpr
  Real x(start = 0.0);
  Real a;
  Real b;
equation
  der(x) = 1.0;
  (a, b) = TestLib.NestedExprOutputs(x);
end MultiOutputNestedExpr;
