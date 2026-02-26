model ArrayConnect
  Component c1;
  Component c2;
equation
  connect(c1.p[1], c2.p[2]);
  c1.p[2].v = 10.0;
  c2.p[1].v = 5.0;
  c1.v_diff = 2.0;
end ArrayConnect;
