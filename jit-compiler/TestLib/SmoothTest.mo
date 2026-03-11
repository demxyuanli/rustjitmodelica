model SmoothTest
  Real x(start = 1.0);
equation
  der(x) = -smooth(x);
end SmoothTest;
