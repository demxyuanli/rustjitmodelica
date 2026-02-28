model IfEqTest
  Real x(start=0.0);
  Real y;
  parameter Real limit = 0.5;
equation
  der(x) = 1.0;
  if x > limit then
    y = 1.0;
  else
    y = 0.0;
  end if;
end IfEqTest;
