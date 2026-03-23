model ClockedTwoRates
  Real y1(start = 0);
  Real y2(start = 0);
  discrete Real c1(start = 0);
  discrete Real c2(start = 0);
equation
  y1 = c1;
  y2 = c2;
  when sample(0.25) then
    c1 = pre(c1) + 1.0;
  end when;
  when sample(0.5) then
    c2 = pre(c2) + 1.0;
  end when;
end ClockedTwoRates;
