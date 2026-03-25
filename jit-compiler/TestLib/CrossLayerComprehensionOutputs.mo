function CrossLayerComprehensionOutputs
  input Real x;
  output Real v;
  output Real s;
protected
  Real base[2];
algorithm
  base := {x, x + 1.0};
  v := {base[i] + x for i in 1:2};
  s := x + 10.0;
end CrossLayerComprehensionOutputs;
