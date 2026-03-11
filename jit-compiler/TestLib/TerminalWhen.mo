model TerminalWhen
  Real x(start = 0);
  Real flag(start = 0);
equation
  der(x) = 1;
  when terminal() then
    flag = 1;
  end when;
end TerminalWhen;
