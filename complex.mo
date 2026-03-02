model ComplexMath
  Real x;
  Real y;
  Real z;
  Real w;
equation
  x = 3.1415926;
  y = sin(x / 2.0);
  z = y * cos(x) + sqrt(4.0);
  w = exp(1.0);
end ComplexMath;
