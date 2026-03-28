model MatrixOuterProduct
  Real y;
equation
  y = outerProduct({2.0, 3.0}, {4.0, 5.0});
end MatrixOuterProduct;
