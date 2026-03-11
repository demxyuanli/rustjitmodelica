model MathBuiltins
  Real x(start = -1.5);
  Real y(start = 2.7);
  Real a;
  Real b;
  Real c;
  Real d;
  Real e;
  Real f;
  Real g;
  Real h;
  Real i;
  Real j;
  Real k;
equation
  der(x) = -x;
  der(y) = 0.1 * y;
  a = abs(x);
  b = sign(x);
  c = sqrt(max(y, 0.01));
  d = min(x, y);
  e = max(x, y);
  f = mod(3.7, 2.0);
  g = rem(3.7, 2.0);
  h = div(7.0, 2.0);
  i = ceil(y);
  j = floor(y);
  k = integer(y);
end MathBuiltins;
