model AdaptiveRKTest
  Real x(start = 1);
equation
  der(x) = -x;
end AdaptiveRKTest;

