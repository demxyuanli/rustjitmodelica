block Constant
  extends Modelica.Blocks.Interfaces.SO;
  parameter Real k = 1.0;
equation
  y.signal = k;
end Constant;
