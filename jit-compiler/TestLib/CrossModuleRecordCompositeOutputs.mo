function CrossModuleRecordCompositeOutputs
  input Real x;
  output Point p;
  output Real s;
protected
  Real tmp[2];
algorithm
  tmp := {Modelica.Math.sin(x + i * Modelica.Constants.pi) for i in 1:2};
  p.x := tmp[1];
  p.y := tmp[2];
  s := x + 30.0;
end CrossModuleRecordCompositeOutputs;
