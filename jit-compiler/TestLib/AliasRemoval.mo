model AliasRemoval
  Real x;
  Real y;
  Real z(start = 1.0);
equation
  x = y;
  y = 1.0;
  der(z) = -z;
end AliasRemoval;
