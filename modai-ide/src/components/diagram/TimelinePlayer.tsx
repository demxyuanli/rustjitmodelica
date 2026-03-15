import { useState, useCallback, useRef, useEffect } from "react";
import { Play, Pause } from "lucide-react";
import { t } from "../../i18n";
import type { StepState } from "../../api/tauri";

interface TimelinePlayerProps {
  stepHistory: StepState[];
  currentStepIndex: number;
  onSeek: (stepIndex: number) => void;
  variableNames?: string[];
}

export function TimelinePlayer({
  stepHistory,
  currentStepIndex,
  onSeek,
  variableNames = [],
}: TimelinePlayerProps) {
  const [isPlaying, setIsPlaying] = useState(false);
  const [playbackSpeed, setPlaybackSpeed] = useState(1);
  const playRef = useRef(false);
  const idxRef = useRef(currentStepIndex);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  idxRef.current = currentStepIndex;

  useEffect(() => {
    if (!isPlaying) return;
    playRef.current = true;
    let frame: number;
    let lastTime = performance.now();
    const interval = 50 / playbackSpeed;

    const tick = (now: number) => {
      if (!playRef.current) return;
      if (now - lastTime >= interval) {
        lastTime = now;
        const next = idxRef.current + 1;
        if (next >= stepHistory.length) {
          setIsPlaying(false);
          playRef.current = false;
          return;
        }
        onSeek(next);
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => {
      playRef.current = false;
      cancelAnimationFrame(frame);
    };
  }, [isPlaying, playbackSpeed, stepHistory.length, onSeek]);

  const togglePlay = useCallback(() => {
    setIsPlaying((prev) => !prev);
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || stepHistory.length === 0 || variableNames.length === 0) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const w = canvas.width;
    const h = canvas.height;
    ctx.clearRect(0, 0, w, h);

    const colors = ["#3b82f6", "#ef4444", "#10b981", "#f59e0b", "#8b5cf6"];

    for (let vi = 0; vi < Math.min(variableNames.length, 5); vi++) {
      const varName = variableNames[vi];
      ctx.strokeStyle = colors[vi % colors.length];
      ctx.lineWidth = 1;
      ctx.beginPath();

      let minVal = Infinity;
      let maxVal = -Infinity;
      const vals: number[] = [];
      for (const s of stepHistory) {
        const idx = s.stateNames.indexOf(varName);
        const val = idx >= 0 ? s.states[idx] : (s.outputNames.indexOf(varName) >= 0 ? s.outputs[s.outputNames.indexOf(varName)] : NaN);
        vals.push(val);
        if (!isNaN(val)) {
          minVal = Math.min(minVal, val);
          maxVal = Math.max(maxVal, val);
        }
      }
      if (maxVal === minVal) maxVal = minVal + 1;
      const range = maxVal - minVal;

      for (let i = 0; i < vals.length; i++) {
        const x = (i / Math.max(1, vals.length - 1)) * w;
        const y = isNaN(vals[i]) ? h / 2 : h - ((vals[i] - minVal) / range) * h;
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();
    }

    if (currentStepIndex >= 0 && currentStepIndex < stepHistory.length) {
      const x = (currentStepIndex / Math.max(1, stepHistory.length - 1)) * w;
      ctx.strokeStyle = "var(--primary, #6366f1)";
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, h);
      ctx.stroke();
    }
  }, [stepHistory, currentStepIndex, variableNames]);

  if (stepHistory.length === 0) {
    return (
      <div className="text-[10px] text-[var(--text-muted)] p-2">
        {t("noSimData")}
      </div>
    );
  }

  const timeRange = stepHistory.length > 1
    ? `${stepHistory[0].time.toFixed(2)} - ${stepHistory[stepHistory.length - 1].time.toFixed(2)}`
    : stepHistory[0]?.time.toFixed(4) ?? "";

  return (
    <div className="flex flex-col gap-1 p-2 bg-[var(--bg-elevated)] border border-[var(--border)] rounded">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide">
          {t("timelinePlayer")}
        </span>
        <span className="text-[10px] text-[var(--text-muted)]">
          {timeRange} ({stepHistory.length} steps)
        </span>
      </div>

      <canvas
        ref={canvasRef}
        width={400}
        height={60}
        className="w-full h-[60px] rounded bg-black/20 cursor-pointer"
        onClick={(e) => {
          const rect = e.currentTarget.getBoundingClientRect();
          const pct = (e.clientX - rect.left) / rect.width;
          const idx = Math.round(pct * (stepHistory.length - 1));
          onSeek(Math.max(0, Math.min(stepHistory.length - 1, idx)));
        }}
      />

      <div className="flex items-center gap-2">
        <input
          type="range"
          min={0}
          max={stepHistory.length - 1}
          value={currentStepIndex}
          onChange={(e) => onSeek(parseInt(e.target.value, 10))}
          className="flex-1 h-1 accent-primary"
        />
        <span className="text-[10px] text-[var(--text)] font-mono w-16 text-right">
          {stepHistory[currentStepIndex]?.time.toFixed(4) ?? "-"}
        </span>
      </div>

      <div className="flex items-center gap-2">
        <button
          type="button"
          className={`p-1.5 rounded ${
            isPlaying
              ? "bg-yellow-600/20 text-yellow-400"
              : "bg-green-600/20 text-green-400"
          }`}
          onClick={togglePlay}
          title={isPlaying ? t("pauseSim") : t("resumeSim")}
        >
          {isPlaying ? <Pause className="h-4 w-4" /> : <Play className="h-4 w-4" />}
        </button>
        <div className="flex items-center gap-1">
          <span className="text-[10px] text-[var(--text-muted)]">{t("playbackSpeed")}:</span>
          {[0.5, 1, 2, 5].map((speed) => (
            <button
              key={speed}
              type="button"
              className={`px-1 py-0.5 rounded text-[9px] ${
                playbackSpeed === speed
                  ? "bg-primary/20 text-primary"
                  : "text-[var(--text-muted)] hover:text-[var(--text)]"
              }`}
              onClick={() => setPlaybackSpeed(speed)}
            >
              {speed}x
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
