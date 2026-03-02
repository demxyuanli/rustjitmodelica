model SimplePendulum
  parameter Real L(start=1.0);
  parameter Real g(start=9.81);
  Real theta(start=1.0);
  Real omega(start=0.0);
equation
  der(theta) = omega;
  der(omega) = - (g / L) * sin(theta);
end SimplePendulum;
