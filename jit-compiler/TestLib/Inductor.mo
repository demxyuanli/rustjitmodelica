model Inductor
  extends TwoPin;
  parameter Real L = 1.0;
equation
  v = L * der(i);
end Inductor;
