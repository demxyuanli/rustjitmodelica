model NestedDer
  Real x(start = 0.0);
  Real y(start = 0.0);
equation
  der(x + y) = 1.0;
  x = y;
end NestedDer;
