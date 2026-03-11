model LoopConnect
  Component c1;
  Component c2;
equation
  for i in 1:2 loop
    connect(c1.p[i], c2.p[i]);
  end for;
  c1.p[1].v = 10.0;
  c1.p[2].v = 20.0;
end LoopConnect;
