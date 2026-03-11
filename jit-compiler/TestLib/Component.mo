model Component
  Pin p[2];
  Real v_diff;
equation
  v_diff = p[1].v - p[2].v;
  p[1].i = 0.0;
  p[2].i = 0.0;
end Component;
