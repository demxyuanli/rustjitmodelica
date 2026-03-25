model ClockedStartAndShiftTest
  Real x(start = 0);
  discrete Real d(start = 0);
equation
  der(x) = 1.0;
  when shiftSample(sample(0.25, 0.5), 1) then
    d = pre(d) + 1.0;
  end when;
end ClockedStartAndShiftTest;
