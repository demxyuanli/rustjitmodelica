function ArrayAndScalarOutputs
  input Real x;
  output Real vec[2];
  output Real s;
algorithm
  vec := {x, x + 1.0};
  s := x * 2.0;
end ArrayAndScalarOutputs;
