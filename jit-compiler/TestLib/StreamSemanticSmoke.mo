model StreamSemanticSmoke
  Real x(start=1.0);
  Real y;
  Real z;
equation
  der(x) = -x;
  y = inStream(x);
  z = actualStream(y);
end StreamSemanticSmoke;
