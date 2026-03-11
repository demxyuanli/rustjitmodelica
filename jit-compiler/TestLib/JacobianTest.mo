model JacobianTest
  Real x(start = 1);
  Real y(start = 0);
equation
  // Simple coupled linear ODE system to test ODE Jacobian
  der(x) = -2 * x + 3 * y;
  der(y) = 1 * x - 1 * y;
end JacobianTest;

