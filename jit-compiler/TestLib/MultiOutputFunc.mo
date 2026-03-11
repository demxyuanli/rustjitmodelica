model MultiOutputFunc
  Real x;
  Real a;
  Real b;
equation
  der(x) = 1.0;
  (a, b) = TestLib.TwoOutputs(x);
end MultiOutputFunc;
