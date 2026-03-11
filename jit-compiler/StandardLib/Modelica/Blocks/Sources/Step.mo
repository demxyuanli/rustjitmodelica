block Step
  extends Modelica.Blocks.Interfaces.SO;
  parameter Real height = 1.0;
  parameter Real offset = 0.0;
  parameter Real startTime = 0.0;
equation
  y.signal = offset + (if time < startTime then 0.0 else height);
end Step;
