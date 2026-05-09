expandable connector C
end C;

model Container
  C c;
end Container;

model Source
  Real x = 1.0;
  Real y = 2.0;
end Source;

model expandable_basic
  Container cont;
  Source src;
equation
  connect(cont.c, src);
end ExpandableTest;
