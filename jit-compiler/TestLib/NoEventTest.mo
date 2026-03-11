model NoEventTest
  Real x(start = -1);
  Real y;
equation
  y = if noEvent(x > 0) then 1 else -1;
  der(x) = 1;
end NoEventTest;

