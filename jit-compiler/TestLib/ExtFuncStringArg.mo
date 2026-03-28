function printStringExternal
  "External function that prints a string - tests string ABI (const char*)"
  input String msg;
  output Real result;
  external "C" result = rustmodlica_print_string(msg);
end printStringExternal;
