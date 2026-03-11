model SIunitsTest
  Modelica.SIunits.Time t(start=1.0);
equation
  der(t) = -t;
end SIunitsTest;
