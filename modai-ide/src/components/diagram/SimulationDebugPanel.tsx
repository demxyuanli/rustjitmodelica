import { useCallback, useMemo } from "react";
import { Play, Pause, SkipForward, Square, RotateCcw } from "lucide-react";
import { t } from "../../i18n";
import type { UseStepDebugResult } from "../../hooks/useStepDebug";

interface SimulationDebugPanelProps {
  debug: UseStepDebugResult;
  onStartDebug: () => void;
}

export function SimulationDebugPanel({ debug, onStartDebug }: SimulationDebugPanelProps) {
  const { state, step, stepN, play, pause, stop, reset } = debug;
  const { status, currentStep, stepHistory, error, totalSteps } = state;

  const isIdle = status === "idle";
  const isRunning = status === "running";
  const isPaused = status === "paused";
  const isCompleted = status === "completed";
  const isError = status === "error";
  const isCompiling = status === "compiling";

  const handlePlay = useCallback(() => {
    if (isIdle) {
      onStartDebug();
    } else if (isPaused) {
      play();
    }
  }, [isIdle, isPaused, play, onStartDebug]);

  const progressPct = useMemo(() => {
    if (totalSteps === 0 || !currentStep) return 0;
    return Math.min(100, (currentStep.stepIndex / Math.max(1, totalSteps)) * 100);
  }, [currentStep, totalSteps]);

  return (
    <div className="flex flex-col gap-2 p-2 bg-[var(--bg-elevated)] border border-[var(--border)] rounded">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide">
          {t("stepDebug")}
        </span>
        <span className={`text-[10px] px-1.5 py-0.5 rounded ${
          isRunning ? "bg-green-500/20 text-green-400" :
          isPaused ? "bg-yellow-500/20 text-yellow-400" :
          isCompleted ? "bg-blue-500/20 text-blue-400" :
          isError ? "bg-red-500/20 text-red-400" :
          isCompiling ? "bg-purple-500/20 text-purple-400" :
          "bg-white/5 text-[var(--text-muted)]"
        }`}>
          {status}
        </span>
      </div>

      <div className="flex items-center gap-1">
        {(isIdle || isPaused) && (
          <button
            type="button"
            className="p-1.5 rounded bg-green-600/20 text-green-400 hover:bg-green-600/30 disabled:opacity-50"
            onClick={handlePlay}
            disabled={isCompiling}
            title={isIdle ? t("simulate") : t("resumeSim")}
          >
            <Play className="h-4 w-4" />
          </button>
        )}
        {isRunning && (
          <button
            type="button"
            className="p-1.5 rounded bg-yellow-600/20 text-yellow-400 hover:bg-yellow-600/30"
            onClick={pause}
            title={t("pauseSim")}
          >
            <Pause className="h-4 w-4" />
          </button>
        )}
        {isPaused && (
          <>
            <button
              type="button"
              className="p-1.5 rounded bg-blue-600/20 text-blue-400 hover:bg-blue-600/30"
              onClick={step}
              title={t("stepForward")}
            >
              <SkipForward className="h-4 w-4" />
            </button>
            <button
              type="button"
              className="p-1.5 rounded bg-blue-600/20 text-blue-400 hover:bg-blue-600/30"
              onClick={() => stepN(10)}
              title={t("stepForward10")}
            >
              <SkipForward className="h-4 w-4" />
            </button>
          </>
        )}
        {!isIdle && (
          <button
            type="button"
            className="p-1.5 rounded bg-red-600/20 text-red-400 hover:bg-red-600/30"
            onClick={stop}
            title={t("stopSim")}
          >
            <Square className="h-4 w-4" />
          </button>
        )}
        {isCompleted && (
          <button
            type="button"
            className="p-1.5 rounded bg-white/5 text-[var(--text-muted)] hover:bg-white/10"
            onClick={reset}
            title={t("resetSim")}
          >
            <RotateCcw className="h-4 w-4" />
          </button>
        )}
      </div>

      {currentStep && (
        <>
          <div className="flex items-center gap-2 text-[10px]">
            <span className="text-[var(--text-muted)]">{t("simTime")}:</span>
            <span className="text-[var(--text)] font-mono">{currentStep.time.toFixed(4)}</span>
            <span className="text-[var(--text-muted)] ml-2">Step:</span>
            <span className="text-[var(--text)] font-mono">{currentStep.stepIndex}</span>
          </div>

          <div className="w-full h-1.5 bg-white/5 rounded overflow-hidden">
            <div
              className="h-full bg-primary transition-all duration-100"
              style={{ width: `${progressPct}%` }}
            />
          </div>

          <div className="max-h-[200px] overflow-auto">
            <div className="text-[10px] text-[var(--text-muted)] mb-1 font-medium">{t("watchVariables")}</div>
            <table className="w-full text-[10px]">
              <tbody>
                {currentStep.stateNames.map((name, i) => (
                  <tr key={name} className="border-b border-[var(--border)]/20">
                    <td className="py-0.5 pr-2 text-[var(--text-muted)]">{name}</td>
                    <td className="py-0.5 text-[var(--text)] font-mono text-right">
                      {currentStep.states[i]?.toPrecision(6) ?? "-"}
                    </td>
                  </tr>
                ))}
                {currentStep.outputNames.map((name, i) => (
                  <tr key={name} className="border-b border-[var(--border)]/20">
                    <td className="py-0.5 pr-2 text-blue-400">{name}</td>
                    <td className="py-0.5 text-[var(--text)] font-mono text-right">
                      {currentStep.outputs[i]?.toPrecision(6) ?? "-"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}

      {error && (
        <div className="text-[10px] text-red-400 bg-red-500/10 rounded px-2 py-1">
          {error}
        </div>
      )}

      {stepHistory.length > 0 && (
        <div className="text-[10px] text-[var(--text-muted)]">
          {stepHistory.length} steps recorded
        </div>
      )}
    </div>
  );
}
