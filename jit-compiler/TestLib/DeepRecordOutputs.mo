function DeepRecordOutputs
  input Real x;
  output DeepOuter o;
  output Real s;
algorithm
  o.inner[1].p[1].x := x + 1.0;
  o.inner[1].p[1].y := x + 2.0;
  o.inner[2].p[2].x := x + 3.0;
  o.inner[2].p[2].y := x + 4.0;
  s := x * 6.0;
end DeepRecordOutputs;
