model MSLBlocksTest
  Modelica.Blocks.Sources.Constant const(k=5.0);
  Modelica.Blocks.Sources.Step step(height=2.0, startTime=1.0);
  Real y_sum;
equation
  y_sum = const.y.signal + step.y.signal;
end MSLBlocksTest;
