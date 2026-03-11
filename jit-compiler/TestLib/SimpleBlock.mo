block SimpleBlock
  input Real u;
  output Real y;
  parameter Real k = 1.0;
equation
  y = k * u;
end SimpleBlock;
