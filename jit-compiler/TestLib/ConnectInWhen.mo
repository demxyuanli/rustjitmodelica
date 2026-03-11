model ConnectInWhen
  Real x(start = 0);
  Real y(start = 0);
  Real z(start = 0);
equation
  der(x) = 1.0;
  when time > 0.5 then
    connect(x, y);
  end when;
  when time > 1.5 then
    connect(y, z);
  end when;
end ConnectInWhen;
