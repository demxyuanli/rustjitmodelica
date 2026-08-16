model StateGraphFirst
  inner Modelica.StateGraph.StateGraphRoot stateGraphRoot;
  Modelica.StateGraph.InitialStep initialStep(nIn = 1, nOut = 1);
  Modelica.StateGraph.Transition transition1(condition = true, enableTimer = true, waitTime = 1);
  Modelica.StateGraph.Step step(nIn = 1, nOut = 1);
  Modelica.StateGraph.Transition transition2(condition = true, enableTimer = true, waitTime = 1);
equation
  connect(initialStep.outPort[1], transition1.inPort);
  connect(transition1.outPort, step.inPort[1]);
  connect(step.outPort[1], transition2.inPort);
  connect(transition2.outPort, initialStep.inPort[1]);
  annotation(experiment(StopTime = 5.5, Interval = 0.011));
end StateGraphFirst;
