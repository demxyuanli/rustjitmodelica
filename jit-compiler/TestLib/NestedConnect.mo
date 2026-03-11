model NestedConnect
  MainPin p1;
  MainPin p2;
equation
  connect(p1, p2);
  p1.val = 10.0;
  p1.s.v = 5.0;
  p1.s.i = 1.0;
end NestedConnect;
