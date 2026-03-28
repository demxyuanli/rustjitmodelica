model ExtFuncArrayArgTest
  "Test model for external function with array argument"
  Real total;
algorithm
  total := TestLib.sumArrayExternal({1.0, 2.0, 3.0, 4.0, 5.0});
end ExtFuncArrayArgTest;
