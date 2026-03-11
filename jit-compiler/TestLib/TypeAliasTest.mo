model TypeAliasTest
  type MyReal = Real;
  MyReal x(start = 1.0);
  parameter MyReal a = -1.0;
equation
  der(x) = a * x;
end TypeAliasTest;
