model MultiAssignRecord
  Real x(start = 0.0);
  Real px;
  Real py;
equation
  der(x) = 1.0;
algorithm
  (px, py) := Point(x + 1.0, x + 2.0);
end MultiAssignRecord;
