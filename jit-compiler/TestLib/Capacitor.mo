model Capacitor
  extends TwoPin;
  parameter Real C = 1.0;
equation
  i = C * der(v);
end Capacitor;
