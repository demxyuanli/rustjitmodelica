function ComprehensionAndScalarOutputs
  input Real x;
  output Real v;
  output Real s;
algorithm
  v := {x + i for i in 1:2};
  s := x + 10.0;
end ComprehensionAndScalarOutputs;
