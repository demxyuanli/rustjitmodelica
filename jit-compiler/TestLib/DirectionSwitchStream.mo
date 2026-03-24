model DirectionSwitchStream
  Real port_a_m_flow;
  Real port_b_m_flow;
  Real port_a_h_outflow;
  Real port_b_h_outflow;
  Real y;
equation
  port_a_m_flow = sin(time);
  port_b_m_flow = -port_a_m_flow;
  port_a_h_outflow = 300 + 10*time;
  port_b_h_outflow = 350 - 5*time;
  y = actualStream(port_a_h_outflow) + inStream(port_a_h_outflow);
end DirectionSwitchStream;
