model MSLTransferFunctionTest
  Modelica.Blocks.Sources.Constant const(k=1.0);
  Modelica.Blocks.Continuous.TransferFunction tf(b=1.0, a=1.0);
  Real y_out;
equation
  connect(const.y, tf.u);
  y_out = tf.y.signal;
end MSLTransferFunctionTest;
