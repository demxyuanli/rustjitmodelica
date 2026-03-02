model SolvableBlock4Res
  Real x;
  Real y;
  Real z;
  Real w;
equation
  x + y + z + w = 10;
  x - y + z - w = 0;
  x + y - z - w = 2;
  x - y - z + w = 0;
end SolvableBlock4Res;
