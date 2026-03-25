model ReinitInWhenTest
  Real x(start = 1);
  Real v(start = 0);
  discrete Real c(start = 0);
equation
  der(x) = v;
  der(v) = -1.0;
  when x < 0 then
    reinit(x, 0.5);
    reinit(v, -pre(v));
    c = pre(c) + 1.0;
  end when;
end ReinitInWhenTest;

