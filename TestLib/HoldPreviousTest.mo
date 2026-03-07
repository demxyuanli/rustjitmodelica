model HoldPreviousTest
  Real x(start = 0);
  discrete Real d(start = 0);
  discrete Real y(start = 0);
equation
  der(x) = 1.0;
  when sample(0.5) then
    y = hold(x);
    d = previous(d) + 1.0;
  end when;
end HoldPreviousTest;
