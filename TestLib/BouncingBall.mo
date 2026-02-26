model BouncingBall
  parameter Real e = 0.7 "Coefficient of restitution";
  parameter Real g = 9.81;
  Real h(start=1.0);
  Real v(start=0.0);
equation
  der(h) = v;
  der(v) = -g;
  
  when h <= 0.0 and v < 0.0 then
    reinit(v, -e * pre(v));
  end when;
end BouncingBall;
