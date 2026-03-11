model InitWithParam
  parameter Real a = 2;
  Real x(start = 0);
initial equation
  x = a + 3;
equation
  der(x) = -x;
end InitWithParam;

