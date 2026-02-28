model AssertTerminateTest
  Real x(start=0.0);
equation
  der(x) = 1.0;
  assert(x >= 0.0, 1.0);
  when x > 5.0 then
    terminate(5.0);
  end when;
end AssertTerminateTest;
