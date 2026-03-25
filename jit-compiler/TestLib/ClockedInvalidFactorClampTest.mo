model ClockedInvalidFactorClampTest
  Real x(start = 0);
  discrete Real a(start = 0);
  discrete Real b(start = 0);
equation
  der(x) = 1.0;
  when subSample(sample(0.0, 0.5), 0) then
    a = pre(a) + 1.0;
  end when;
  when superSample(sample(0.0, 0.5), -2) then
    b = pre(b) + 1.0;
  end when;
end ClockedInvalidFactorClampTest;

