function ExtLibFunc
  input Real x;
  output Real y;
  external "C" annotation(Library = "mylib");
end ExtLibFunc;
