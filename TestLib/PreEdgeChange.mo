model PreEdgeChange
  discrete Real d(start = 0);
  Real x(start = 0);
  output Real pre_d_out;
  output Real d_out;
equation
  der(x) = 1.0;
  when time >= 1.0 then
    d = 1.0;
  end when;
  pre_d_out = pre(d);
  d_out = d;
end PreEdgeChange;
