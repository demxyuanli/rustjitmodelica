function sumArrayExternal
  "External function that sums an array - tests array ABI (ptr+size)"
  input Real u[:];
  output Real sum;
  external "C" sum = rustmodlica_sum_array(u, size(u, 1));
end sumArrayExternal;
