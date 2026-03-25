model ClockedStartShiftThenBackSampleTest
  Real x(start = 0);
  discrete Real d(start = 0);
equation
  der(x) = 1.0;
  when backSample(shiftSample(sample(0.2, 0.4), 1), 2) then
    d = pre(d) + 1.0;
  end when;
end ClockedStartShiftThenBackSampleTest;
