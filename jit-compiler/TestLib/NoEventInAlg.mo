model NoEventInAlg
  Real x(start = -1);
  Real y(start = 0);
algorithm
  if noEvent(x > 0) then
    y := 1;
  else
    y := -1;
  end if;
equation
  der(x) = 1;
end NoEventInAlg;
