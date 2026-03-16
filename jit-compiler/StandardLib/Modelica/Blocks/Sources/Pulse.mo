block Pulse
  extends Modelica.Blocks.Interfaces.SO;
  parameter Real amplitude = 1.0;
  parameter Real width(final min = 1e-6, final max = 100.0) = 0.5;
  parameter Real period = 1.0;
  parameter Real offset = 0.0;
  parameter Real startTime = 0.0;
equation
  y.signal = offset + (if time < startTime then 0.0 else (if mod(time - startTime, period) < period * width then amplitude else 0.0));
end Pulse;
