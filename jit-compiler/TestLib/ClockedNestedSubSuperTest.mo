model ClockedNestedSubSuperTest
  Real x(start = 0);
  discrete Real d(start = 0);
equation
  der(x) = 1.0;
  when superSample(subSample(sample(0.25), 2), 2) then
    d = pre(d) + 1.0;
  end when;
end ClockedNestedSubSuperTest;
