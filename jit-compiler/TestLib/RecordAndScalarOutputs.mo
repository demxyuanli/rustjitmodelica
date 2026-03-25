function RecordAndScalarOutputs
  input Real x;
  output Point p;
  output Real s;
algorithm
  p.x := x + 1.0;
  p.y := x + 2.0;
  s := x * 3.0;
end RecordAndScalarOutputs;
