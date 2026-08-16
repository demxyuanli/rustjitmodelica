model GateSubsystem
  Modelica.Blocks.Sources.Step step1(height = 1, startTime = 0);
  Modelica.Blocks.Interfaces.RealInput plant_in1;
  Modelica.Blocks.Math.Gain plant_gain1(k = 2);
  Modelica.Blocks.Interfaces.RealOutput plant_out1;
equation
  connect(plant_in1.y, plant_gain1.u);
  connect(plant_gain1.y, plant_out1.u);
  connect(step1.y, plant.In1);
end GateSubsystem;
