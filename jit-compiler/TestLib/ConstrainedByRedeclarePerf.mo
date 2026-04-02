model ConstrainedByRedeclarePerf
  model Parent
    partial model Base
      Real x;
    end Base;

    model Derived
      extends Base;
    equation
      x = 1.0;
    end Derived;

    replaceable Base b constrainedby Derived;
  equation
    b.x = 0.0;
  end Parent;

  extends Parent(redeclare Derived b);
equation
  b.x = 3.0;
end ConstrainedByRedeclarePerf;

