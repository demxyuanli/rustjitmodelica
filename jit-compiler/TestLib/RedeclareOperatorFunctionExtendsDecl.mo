model RedeclareOperatorFunctionExtendsDecl
  replaceable function BaseFun
    input Real u;
    output Real y;
  algorithm
    y := u;
  end BaseFun;

  redeclare operator function extends BaseFun
  algorithm
    y := u;
  end BaseFun;

  Real z;
equation
  z = 0.0;
end RedeclareOperatorFunctionExtendsDecl;
