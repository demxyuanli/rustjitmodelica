model AnnotationParse
  Real x(start = 1.0) annotation(Icon(coordinateSystem(preserveAspectRatio = false)));
equation
  der(x) = -x;
end AnnotationParse annotation(version = "1.0");
