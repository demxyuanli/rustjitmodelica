model InitDummy
  Real x(start = 0);
initial equation
  x = 5;
equation
  der(x) = -x;
end InitDummy;

