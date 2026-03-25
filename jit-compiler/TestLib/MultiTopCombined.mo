function MultiTopCombinedFunc
  input Real x;
  output Real y1;
  output Real y2;
algorithm
  y1 := x + 3.0;
  y2 := x - 2.0;
end MultiTopCombinedFunc;

model MultiTopCombined
  Real x(start = 0.0);
  Real a;
  Real b;
equation
  der(x) = 1.0;
  (a, b) = MultiTopCombinedFunc(x);
end MultiTopCombined;
