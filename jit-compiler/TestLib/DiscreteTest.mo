model DiscreteTest
  discrete Real d(start=0.0);
  Real x(start=0.0);
equation
  der(x) = 1.0;
  
  when x >= 1.0 then
    d = pre(d) + 1.0;
    reinit(x, 0.0);
  end when;
end DiscreteTest;
