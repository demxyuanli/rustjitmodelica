model VoltageSource
  extends TwoPin;
  parameter Real V(start=10.0);
equation
  v = V;
end VoltageSource;
