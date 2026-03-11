model ChildWithMod
  extends Parent(p1 = 5.0);
  Real v2;
equation
  v2 = v + 10.0;
end ChildWithMod;
