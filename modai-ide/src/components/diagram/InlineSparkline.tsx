import { useRef, useEffect, useMemo } from "react";

interface InlineSparklineProps {
  data: number[];
  width?: number;
  height?: number;
  color?: string;
  highlightIndex?: number;
  onClick?: () => void;
}

export function InlineSparkline({
  data,
  width = 80,
  height = 30,
  color = "#3b82f6",
  highlightIndex,
  onClick,
}: InlineSparklineProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  const { minVal, maxVal } = useMemo(() => {
    if (data.length === 0) return { minVal: 0, maxVal: 1 };
    let min = Infinity;
    let max = -Infinity;
    for (const v of data) {
      if (v < min) min = v;
      if (v > max) max = v;
    }
    if (max === min) max = min + 1;
    return { minVal: min, maxVal: max };
  }, [data]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, width, height);

    if (data.length < 2) return;

    const range = maxVal - minVal;
    const pad = 2;
    const drawW = width - pad * 2;
    const drawH = height - pad * 2;

    ctx.strokeStyle = color;
    ctx.lineWidth = 1.2;
    ctx.beginPath();
    for (let i = 0; i < data.length; i++) {
      const x = pad + (i / (data.length - 1)) * drawW;
      const y = pad + drawH - ((data[i] - minVal) / range) * drawH;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();

    if (highlightIndex != null && highlightIndex >= 0 && highlightIndex < data.length) {
      const x = pad + (highlightIndex / (data.length - 1)) * drawW;
      const y = pad + drawH - ((data[highlightIndex] - minVal) / range) * drawH;

      ctx.strokeStyle = "rgba(255,255,255,0.3)";
      ctx.lineWidth = 0.5;
      ctx.beginPath();
      ctx.moveTo(x, pad);
      ctx.lineTo(x, height - pad);
      ctx.stroke();

      ctx.fillStyle = color;
      ctx.beginPath();
      ctx.arc(x, y, 2.5, 0, Math.PI * 2);
      ctx.fill();
    }
  }, [data, width, height, color, minVal, maxVal, highlightIndex]);

  return (
    <canvas
      ref={canvasRef}
      style={{ width, height, cursor: onClick ? "pointer" : "default" }}
      className="rounded bg-black/20"
      onClick={onClick}
    />
  );
}
