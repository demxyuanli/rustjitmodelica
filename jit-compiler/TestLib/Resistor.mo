model Resistor
  extends TwoPin;
  parameter Real R(start=1.0);
equation
  v = i * R;
end Resistor;
