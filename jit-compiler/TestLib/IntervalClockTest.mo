model IntervalClockTest
  Real x(start = 0);
  discrete Real y(start = 0);
equation
  der(x) = 1.0;
  when sample(0.5) then
    y = interval(sample(0.5));
  end when;
end IntervalClockTest;
