function RecordNestedArrayOutputs
  input Real x;
  output Point pts[2];
  output Real s;
algorithm
  pts[1].x := x + 1.0;
  pts[1].y := x + 2.0;
  pts[2].x := x + 3.0;
  pts[2].y := x + 4.0;
  s := x * 5.0;
end RecordNestedArrayOutputs;
