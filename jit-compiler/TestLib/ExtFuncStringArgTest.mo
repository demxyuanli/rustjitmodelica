model ExtFuncStringArgTest
  "Test model for external function with string argument"
  Real result;
algorithm
  result := TestLib.printStringExternal("Hello from Modelica JIT!");
end ExtFuncStringArgTest;
