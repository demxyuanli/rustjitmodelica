package AliasChainPkg
  type AliasA = Point;
  type AliasB = AliasA;

  function AliasChainOutputs
    input Real x;
    output AliasB p;
    output Real s;
  algorithm
    p.x := x + 1.0;
    p.y := x + 2.0;
    s := x + 40.0;
  end AliasChainOutputs;
end AliasChainPkg;
