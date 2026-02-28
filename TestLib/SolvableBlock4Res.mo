model SolvableBlock4Res
  Real x1;
  Real x2;
  Real x3;
  Real x4;
equation
  x1 + x2 + x3 + x4 - 10 = 0;
  x1 - x2 + x3 - x4 - 2 = 0;
  x1 + x2 - x3 - x4 - 2 = 0;
  x1 - x2 - x3 + x4 - 2 = 0;
end SolvableBlock4Res;
