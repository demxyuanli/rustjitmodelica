block Sine
  extends Modelica.Blocks.Interfaces.SO;
  parameter Real amplitude = 1.0;
  parameter Real freqHz = 1.0;
  parameter Real phase = 0.0;
  parameter Real offset = 0.0;
  parameter Real startTime = 0.0;
equation
  y.signal = offset + (if time < startTime then 0.0 else amplitude * Modelica.Math.sin(2.0 * Modelica.Constants.pi * freqHz * (time - startTime) + phase));
end Sine;
