model ElseWhenPriorityTest
  Real x(start = -1);
  discrete Real a(start = 0);
  discrete Real b(start = 0);
equation
  der(x) = 1.0;
  when x > 0 then
    a = pre(a) + 1.0;
  elsewhen x > -0.5 then
    b = pre(b) + 1.0;
  end when;
end ElseWhenPriorityTest;

