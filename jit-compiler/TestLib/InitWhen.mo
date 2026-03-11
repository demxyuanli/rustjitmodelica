model InitWhen
  Real x(start = 0);
equation
  when initial() then
    x = 5;
  end when;
  der(x) = -x;
end InitWhen;

