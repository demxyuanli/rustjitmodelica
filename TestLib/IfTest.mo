model IfTest
  Real x(start=0.0);
  Real y;
  parameter Real limit = 0.5;
equation
  der(x) = 1.0;
  y = if x > limit then 1.0 else 0.0;
end IfTest;
