function factorial
  input Real n;
  output Real y;
algorithm
  y := if n <= 1 then 1.0 else n * factorial(n - 1);
end factorial;
