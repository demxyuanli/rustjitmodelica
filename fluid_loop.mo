model FluidLoop
  parameter Real u(start=1.0);
  parameter Real alpha(start=0.1);
  parameter Real dx(start=0.2);
  parameter Real T_in(start=100.0);

  // Array of 5 elements
  Real T[5](start=20.0);

equation
  // Node 1
  der(T[1]) = -u * (T[1] - T_in) / dx + alpha * (T[2] - 2.0*T[1] + T_in) / (dx*dx);

  // Nodes 2-4 (using for loop)
  for i in 2:4 loop
    der(T[i]) = -u * (T[i] - T[i-1]) / dx + alpha * (T[i+1] - 2.0*T[i] + T[i-1]) / (dx*dx);
  end for;

  // Node 5
  der(T[5]) = -u * (T[5] - T[4]) / dx + alpha * (T[5] - 2.0*T[5] + T[4]) / (dx*dx);
end FluidLoop;
