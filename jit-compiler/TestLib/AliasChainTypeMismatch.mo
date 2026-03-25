model AliasChainTypeMismatch
  Real x(start = 0.0);
  Real a;
  Real b;
equation
  der(x) = 1.0;
algorithm
  (a, b) := TestLib.AliasChainPkg.AliasChainOutputs(x);
end AliasChainTypeMismatch;
