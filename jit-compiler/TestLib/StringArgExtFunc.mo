model StringArgExtFunc
  Real x(start = 0);
equation
  der(x) = extLog("test") - x;
end StringArgExtFunc;
