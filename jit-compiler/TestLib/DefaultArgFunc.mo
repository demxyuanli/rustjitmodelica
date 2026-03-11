function DefaultArgFunc
  input Real x;
  input Real y = 2.0;
  output Real z;
algorithm
  z := x + y;
end DefaultArgFunc;
