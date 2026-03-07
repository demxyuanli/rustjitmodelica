model ReinitTest
  Real x(start = 0);
equation
  der(x) = 1.0;
  when x > 1.0 then
    reinit(x, 0.0);
  end when;
end ReinitTest;
