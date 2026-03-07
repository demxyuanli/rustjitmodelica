model ClockedPartitionTest
  Real x(start = 0);
  discrete Real d(start = 0);
equation
  der(x) = 1.0;
  when sample(0.5) then
    d = pre(d) + 1.0;
  end when;
end ClockedPartitionTest;
