model FuncInline
  Real x(start = 0);
  Real y;
equation
  der(x) = 1;
  y = addOne(x);
end FuncInline;
