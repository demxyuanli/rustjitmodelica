model AlgorithmElseWhen
  Real x(start=0.0);
  discrete Real y(start=0.0);
equation
  der(x) = 1.0;
algorithm
  when x > 0.5 then
    y := 1.0;
  elsewhen x > 1.0 then
    y := 2.0;
  end when;
end AlgorithmElseWhen;
