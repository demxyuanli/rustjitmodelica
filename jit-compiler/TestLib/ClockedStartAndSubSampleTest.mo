model ClockedStartAndSubSampleTest
  Real x(start = 0);
  discrete Real d(start = 0);
equation
  der(x) = 1.0;
  when subSample(sample(0.2, 0.3), 2) then
    d = pre(d) + 1.0;
  end when;
end ClockedStartAndSubSampleTest;
