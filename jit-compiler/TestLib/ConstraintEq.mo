model ConstraintEq
  Real x(start = 1);
  Real y(start = 0);
equation
  der(x) = -x;
  0 = x - y;
end ConstraintEq;
