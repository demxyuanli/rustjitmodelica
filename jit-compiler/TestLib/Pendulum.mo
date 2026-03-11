model Pendulum
  // Index-3 DAE System
  Real x(start=1.0);
  Real y(start=0.0);
  Real vx(start=0.0);
  Real vy(start=0.0);
  Real lambda(start=0.0);
  parameter Real L = 1.0;
  parameter Real g = 9.81;
  parameter Real m = 1.0;
equation
  der(x) = vx;
  der(y) = vy;
  m * der(vx) = -x * lambda;
  m * der(vy) = -y * lambda - m * g;
  x*x + y*y = L*L;
end Pendulum;
