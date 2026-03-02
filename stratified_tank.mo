model StratifiedTank
  parameter Real m_flow(start=0.5); 
  parameter Real cp(start=4180.0);
  parameter Real V_tank(start=1.0);
  parameter Real rho(start=1000.0);
  parameter Real T_in(start=350.0);
  parameter Real h_loss(start=10.0);
  parameter Real T_amb(start=293.15);
  
  parameter Real V_node(start=0.1); // V_tank / 10
  parameter Real m_mass(start=100.0); // V_node * rho
  
  Real T[10](start=300.0);

equation
  // First node (bottom/inlet)
  der(T[1]) = (m_flow * cp * (T_in - T[1]) - h_loss * (T[1] - T_amb)) / (m_mass * cp);
  
  // Middle nodes
  for i in 2:10 loop
    der(T[i]) = (m_flow * cp * (T[i-1] - T[i]) - h_loss * (T[i] - T_amb)) / (m_mass * cp);
  end for;
end StratifiedTank;
