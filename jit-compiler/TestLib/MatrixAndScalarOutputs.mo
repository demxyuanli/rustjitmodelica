function MatrixAndScalarOutputs
  input Real x;
  output Real m;
  output Real s;
algorithm
  m := {{x, x + 1.0}, {x + 2.0, x + 3.0}};
  s := x * 2.0;
end MatrixAndScalarOutputs;
