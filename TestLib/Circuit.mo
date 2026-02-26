model Circuit
  VoltageSource src(V=10);
  Resistor R1(R=10);
  Resistor R2(R=20);
  Ground g;
equation
  connect(src.p, R1.p);
  connect(R1.n, R2.p);
  connect(R2.n, src.n);
  connect(src.n, g.p);
end Circuit;
