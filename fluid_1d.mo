model Fluid1D
  // Parameters
  parameter Real u(start=1.0);      // Velocity (m/s)
  parameter Real alpha(start=0.1);  // Diffusion coefficient
  parameter Real dx(start=0.2);     // Grid spacing (Total length 1.0m / 5 grids)
  parameter Real T_in(start=100.0); // Inlet Temperature

  // States (Temperature at 5 grid nodes)
  Real T1(start=20.0);
  Real T2(start=20.0);
  Real T3(start=20.0);
  Real T4(start=20.0);
  Real T5(start=20.0);

equation
  // Finite Volume Method Discretization
  // Convection: Upwind Scheme
  // Diffusion: Central Difference
  
  // Node 1 (Left boundary T_in)
  der(T1) = -u * (T1 - T_in) / dx + alpha * (T2 - 2.0*T1 + T_in) / (dx*dx);

  // Node 2
  der(T2) = -u * (T2 - T1) / dx + alpha * (T3 - 2.0*T2 + T1) / (dx*dx);

  // Node 3
  der(T3) = -u * (T3 - T2) / dx + alpha * (T4 - 2.0*T3 + T2) / (dx*dx);

  // Node 4
  der(T4) = -u * (T4 - T3) / dx + alpha * (T5 - 2.0*T4 + T3) / (dx*dx);

  // Node 5 (Right boundary: Zero gradient approximation T_right = T5)
  der(T5) = -u * (T5 - T4) / dx + alpha * (T5 - 2.0*T5 + T4) / (dx*dx);
end Fluid1D;
