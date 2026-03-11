model MathTest
  Real x(start=0.5);
  Real y;
  Real z;
  Real w;
  Real v;
  Real u;
  Real s;
  Real c;
  Real t;
  parameter Real p = 2.0;
equation
  der(x) = -sin(time);
  y = cos(x);
  z = tan(x);
  w = exp(x);
  v = log(p);
  u = sqrt(p);
  s = abs(sin(time));
  c = ceil(x);
  t = floor(x);
end MathTest;
