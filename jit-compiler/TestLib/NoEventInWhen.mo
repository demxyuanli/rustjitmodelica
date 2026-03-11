model NoEventInWhen
  Real x(start = -1);
  Real y(start = 0);
equation
  der(x) = 1;
  when noEvent(x > 0.5) then
    y = 1;
  elsewhen noEvent(x <= 0.5) then
    y = 0;
  end when;
end NoEventInWhen;
