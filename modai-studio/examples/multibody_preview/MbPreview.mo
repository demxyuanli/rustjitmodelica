model MbPreview
  inner Modelica.Mechanics.MultiBody.World world(g = 9.81, n = {0,-1,0});
  Modelica.Mechanics.MultiBody.Joints.Revolute revolute(n = {0,0,1}, phi(start = 0.5));
  Modelica.Mechanics.MultiBody.Parts.FixedTranslation rod(r = {0, -0.5, 0});
  Modelica.Mechanics.MultiBody.Parts.Body body(m = 1, r_CM = {0,0,0});
equation
  connect(world.frame_b, revolute.frame_a);
  connect(revolute.frame_b, rod.frame_a);
  connect(rod.frame_b, body.frame_a);
  annotation(experiment(StopTime = 2, Interval = 0.004));
end MbPreview;
