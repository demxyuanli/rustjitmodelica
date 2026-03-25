function CrossModuleComprehensionOutputs
  input Real x;
  output Real v;
  output Real s;
algorithm
  v := {Modelica.Math.sin(x + i * Modelica.Constants.pi) for i in 1:2};
  s := x + 20.0;
end CrossModuleComprehensionOutputs;
