model AlgebraicLoop2Eq
  Real u;
  Real y;
equation
  // Pure algebraic loop with two coupled equations
  y = sin(u);
  u = y;
end AlgebraicLoop2Eq;

