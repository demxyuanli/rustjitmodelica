model ComponentLibraryCoverage
  Modelica.Blocks.Sources.Step step(height=2.0, startTime=0.5);
  Modelica.Blocks.Sources.Sine sine(amplitude=1.5, freqHz=0.5);
  Modelica.Blocks.Continuous.TransferFunction tf(b=2.0, a=1.0);
  Modelica.Blocks.Continuous.TransferFunction tfAux(b=1.0, a=2.0);
  Modelica.Blocks.Continuous.Integrator integrator(k=0.75, y_start=0.25);
  Modelica.Blocks.Continuous.Integrator integratorAux(k=0.5, y_start=0.0);
  
  // Added complex test components
  Modelica.Blocks.Sources.Pulse pulse(amplitude=3.0, width=60, period=1.0);
  Modelica.Blocks.Math.Add add(k1=1.0, k2=-0.5);
  Modelica.Blocks.Math.Product product;
  Modelica.Blocks.Nonlinear.Limiter limiter(uMax=5.0, uMin=-2.0);
  Modelica.Blocks.Continuous.Derivative derivative(T=0.01, k=1.0);
  Modelica.Blocks.Continuous.FirstOrder firstOrder(T=0.1, k=2.0);
  Modelica.Blocks.Logical.GreaterThreshold greaterThreshold(threshold=1.0);
  Modelica.Blocks.Logical.Switch switch1;
  Modelica.Blocks.Sources.Constant const1(k=10.0);
  Modelica.Blocks.Sources.Constant const2(k=-5.0);
  
  VoltageSource src(V=12.0);
  Resistor r1(R=10.0);
  Resistor r2(R=25.0);
  Capacitor cap(C=0.001);
  Inductor ind(L=0.01);
  Ground g;
  
  // Added mechanical system components
  Modelica.Mechanics.Rotational.Components.Inertia inertia(J=0.1);
  Modelica.Mechanics.Rotational.Sources.Torque torque;
  Modelica.Mechanics.Rotational.Components.Spring spring(c=100.0);
  Modelica.Mechanics.Rotational.Components.Damper damper(d=10.0);
  Modelica.Mechanics.Rotational.Sensors.SpeedSensor speedSensor;
  
  // Added thermal system components
  Modelica.Thermal.HeatTransfer.Components.HeatCapacitor heatCapacitor(C=1000);
  Modelica.Thermal.HeatTransfer.Components.ThermalConductor thermalConductor(G=10.0);
  Modelica.Thermal.HeatTransfer.Sources.FixedTemperature fixedTemp(T=293.15);
  Modelica.Thermal.HeatTransfer.Sources.PrescribedHeatFlow prescribedHeat;
  
  // Added control feedback loop
  Modelica.Blocks.Continuous.PID pid(k=1.0, Ti=0.1, Td=0.01);
  Modelica.Blocks.Sources.Ramp ramp(height=50, duration=10);
  Modelica.Blocks.Math.Feedback feedback;
  
equation
  connect(step.y, tf.u);
  connect(tf.y, integrator.u);
  connect(sine.y, tfAux.u);
  connect(tfAux.y, integratorAux.u);
  
  // Added complex signal processing connections
  connect(pulse.y, add.u1);
  connect(sine.y, add.u2);
  connect(add.y, product.u1);
  connect(step.y, product.u2);
  connect(product.y, limiter.u);
  connect(limiter.y, derivative.u);
  connect(derivative.y, firstOrder.u);
  connect(firstOrder.y, greaterThreshold.u);
  connect(greaterThreshold.y, switch1.u2);
  connect(const1.y, switch1.u1);
  connect(const2.y, switch1.u3);
  
  // Added circuit connections
  connect(src.p, r1.p);
  connect(r1.n, r2.p);
  connect(r2.n, cap.p);
  connect(cap.n, ind.p);
  connect(ind.n, src.n);
  connect(src.n, g.p);
  
  // Added mechanical system connections
  connect(torque.flange, inertia.flange_a);
  connect(inertia.flange_b, spring.flange_a);
  connect(spring.flange_b, damper.flange_a);
  connect(damper.flange_b, speedSensor.flange);
  
  // Added thermal system connections
  connect(fixedTemp.port, thermalConductor.port_a);
  connect(thermalConductor.port_b, heatCapacitor.port);
  connect(prescribedHeat.port, heatCapacitor.port);
  
  // Added control system connections
  connect(ramp.y, feedback.u1);
  connect(speedSensor.w, feedback.u2);
  connect(feedback.y, pid.u);
  connect(pid.y, torque.tau);
  
  // Connect heat source control
  connect(switch1.y, prescribedHeat.Q_flow);
end ComponentLibraryCoverage;