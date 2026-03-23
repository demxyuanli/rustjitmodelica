model RealFftJitSmoke
  "Minimal realFFT intrinsic smoke test (fixed array sizes)."
  parameter Integer ns = 8;
  parameter Integer nfi = 5;
  Real y_buf[ns](each start = 0, each fixed = true);
  discrete Real info(start = 0, fixed = true);
  final output Real Ai[nfi](each start = 0, each fixed = true);
  final output Real Phii[nfi](each start = 0, each fixed = true);
algorithm
  when sample(0, 1.0) then
    y_buf[1] := 1.0;
    y_buf[2] := -1.0;
    y_buf[3] := 1.0;
    y_buf[4] := -1.0;
    y_buf[5] := 0.0;
    y_buf[6] := 0.0;
    y_buf[7] := 0.0;
    y_buf[8] := 0.0;
    (info, Ai, Phii) := Modelica.Math.FastFourierTransform.realFFT(y_buf, nfi);
  end when;
end RealFftJitSmoke;
