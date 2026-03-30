import { useState, useCallback, useRef } from "react";
import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  startSimulationSession,
  simulationStep,
  simulationCommand,
  type StepState,
} from "../api/tauri";

export type DebugStatus = "idle" | "compiling" | "running" | "paused" | "completed" | "error";

export interface StepDebugState {
  status: DebugStatus;
  sessionId: string | null;
  currentStep: StepState | null;
  stepHistory: StepState[];
  error: string | null;
  totalSteps: number;
  progressMessage: string | null;
  averageStepsPerSec: number;
  recentStepsPerSec: number;
  controlEvents: string[];
  progressEventCount: number;
}

export interface UseStepDebugResult {
  state: StepDebugState;
  startSession: (code: string, modelName?: string, projectDir?: string | null) => Promise<void>;
  step: () => Promise<void>;
  stepN: (n: number) => Promise<void>;
  play: () => void;
  pause: () => void;
  stop: () => void;
  reset: () => void;
  seekTo: (stepIndex: number) => void;
  getStateAtStep: (stepIndex: number) => StepState | null;
}

export function useStepDebug(): UseStepDebugResult {
  const STEP_PROGRESS_EVERY = 25;
  const STEP_PROGRESS_MIN_INTERVAL_MS = 1000;
  const [state, setState] = useState<StepDebugState>({
    status: "idle",
    sessionId: null,
    currentStep: null,
    stepHistory: [],
    error: null,
    totalSteps: 0,
    progressMessage: null,
    averageStepsPerSec: 0,
    recentStepsPerSec: 0,
    controlEvents: [],
    progressEventCount: 0,
  });

  const playingRef = useRef(false);
  const sessionIdRef = useRef<string | null>(null);
  const runStartedAtRef = useRef<number | null>(null);
  const lastProgressAtRef = useRef(0);
  const lastProgressStepRef = useRef(-1);
  const stepTimestampsRef = useRef<number[]>([]);

  const calcStepRates = useCallback((totalSteps: number, now: number) => {
    const startedAt = runStartedAtRef.current ?? now;
    const elapsedSec = Math.max(0.001, (now - startedAt) / 1000);
    const average = totalSteps / elapsedSec;
    const windowStart = now - 1000;
    stepTimestampsRef.current = stepTimestampsRef.current.filter((ts) => ts >= windowStart);
    const recent = stepTimestampsRef.current.length;
    return {
      averageStepsPerSec: Number(average.toFixed(2)),
      recentStepsPerSec: Number(recent.toFixed(2)),
    };
  }, []);

  const shouldReportStepProgress = useCallback((stepIndex: number) => {
    if (stepIndex <= 0) return false;
    if (stepIndex - lastProgressStepRef.current < STEP_PROGRESS_EVERY) return false;
    const now = Date.now();
    if (now - lastProgressAtRef.current < STEP_PROGRESS_MIN_INTERVAL_MS) return false;
    lastProgressAtRef.current = now;
    lastProgressStepRef.current = stepIndex;
    return true;
  }, []);

  useEffect(() => {
    let disposed = false;
    let off: (() => void) | null = null;
    void listen<{ task?: string; stage?: string; elapsedSec?: number; message?: string }>("modai-jit-progress", (event) => {
      if (disposed) return;
      const p = (event.payload ?? {}) as {
        category?: "control" | "progress" | "error";
        task?: string;
        stage?: string;
        elapsedSec?: number;
        message?: string;
        currentStep?: number;
        totalSteps?: number;
        reason?: string;
      };
      if (p.task !== "start-session" && p.task !== "step-session") return;
      const text = p.message?.trim() || `Step-debug ${p.stage ?? "running"}`;
      const elapsed = p.elapsedSec != null ? ` (${p.elapsedSec}s)` : "";
      const stepPart =
        p.currentStep != null && p.totalSteps != null ? ` [${p.currentStep}/${p.totalSteps}]` : "";
      const reasonPart = p.reason ? ` reason=${p.reason}` : "";
      const timelineText = `${text}${elapsed}${stepPart}${reasonPart}`;
      setState((prev) => {
        if (p.category === "progress") {
          return {
            ...prev,
            progressMessage: text,
            progressEventCount: prev.progressEventCount + 1,
          };
        }
        if (p.category === "control" || p.category === "error") {
          const prefix = p.category === "error" ? "ERROR" : "CTRL";
          const nextEvents = [`${prefix} ${timelineText}`, ...prev.controlEvents].slice(0, 20);
          return {
            ...prev,
            progressMessage: text,
            controlEvents: nextEvents,
          };
        }
        return { ...prev, progressMessage: text };
      });
    }).then((u) => {
      if (disposed) u();
      else off = u;
    }).catch(() => {});
    return () => {
      disposed = true;
      off?.();
    };
  }, []);

  const startSession = useCallback(
    async (code: string, modelName?: string, projectDir?: string | null) => {
      runStartedAtRef.current = Date.now();
      lastProgressAtRef.current = 0;
      lastProgressStepRef.current = -1;
      stepTimestampsRef.current = [];
      setState((prev) => ({ ...prev, status: "compiling", error: null, stepHistory: [], currentStep: null, progressMessage: "Starting step-debug session..." }));
      setState((prev) => ({ ...prev, controlEvents: [], progressEventCount: 0 }));
      try {
        const sid = await startSimulationSession(code, modelName, projectDir);
        sessionIdRef.current = sid;
        setState((prev) => ({
          ...prev,
          status: "paused",
          sessionId: sid,
          error: null,
          progressMessage: "Step-debug session ready.",
        }));
      } catch (err) {
        setState((prev) => ({
          ...prev,
          status: "error",
          error: String(err),
          progressMessage: null,
        }));
      }
    },
    [],
  );

  const step = useCallback(async () => {
    const sid = sessionIdRef.current;
    if (!sid) return;
    try {
      const stepState = await simulationStep(sid);
      const now = Date.now();
      stepTimestampsRef.current.push(now);
      setState((prev) => ({
        ...prev,
        status: "paused",
        currentStep: stepState,
        stepHistory: [...prev.stepHistory, stepState],
        totalSteps: stepState.stepIndex + 1,
        progressMessage: `Step ${stepState.stepIndex + 1} recorded.`,
        ...calcStepRates(stepState.stepIndex + 1, now),
      }));
    } catch (err) {
      const errMsg = String(err);
      if (errMsg.includes("completed")) {
        const elapsed = runStartedAtRef.current != null ? Math.max(1, Math.floor((Date.now() - runStartedAtRef.current) / 1000)) : 0;
        setState((prev) => ({ ...prev, status: "completed", progressMessage: `Step completed in ${elapsed}s.` }));
      } else {
        setState((prev) => ({ ...prev, status: "error", error: errMsg }));
      }
    }
  }, []);

  const stepN = useCallback(
    async (n: number) => {
      const batchStarted = Date.now();
      setState((prev) => ({ ...prev, progressMessage: `Running ${n} debug steps...` }));
      for (let i = 0; i < n; i++) {
        const sid = sessionIdRef.current;
        if (!sid) break;
        try {
          const stepState = await simulationStep(sid);
          const now = Date.now();
          stepTimestampsRef.current.push(now);
          setState((prev) => ({
            ...prev,
            status: "paused",
            currentStep: stepState,
            stepHistory: [...prev.stepHistory, stepState],
            totalSteps: stepState.stepIndex + 1,
            progressMessage:
              shouldReportStepProgress(stepState.stepIndex + 1) ?
                `StepN progress: ${stepState.stepIndex + 1} steps completed`
              : prev.progressMessage,
            ...calcStepRates(stepState.stepIndex + 1, now),
          }));
        } catch {
          const elapsed = Math.max(1, Math.floor((Date.now() - batchStarted) / 1000));
          setState((prev) => ({ ...prev, status: "completed", progressMessage: `StepN completed in ${elapsed}s.` }));
          break;
        }
      }
    },
    [shouldReportStepProgress],
  );

  const play = useCallback(() => {
    playingRef.current = true;
    runStartedAtRef.current = Date.now();
    lastProgressAtRef.current = 0;
    lastProgressStepRef.current = -1;
    stepTimestampsRef.current = [];
    setState((prev) => ({ ...prev, status: "running", progressMessage: "Playback started..." }));
    const tick = async () => {
      if (!playingRef.current || !sessionIdRef.current) return;
      try {
        const stepState = await simulationStep(sessionIdRef.current);
        const now = Date.now();
        stepTimestampsRef.current.push(now);
        setState((prev) => ({
          ...prev,
          currentStep: stepState,
          stepHistory: [...prev.stepHistory, stepState],
          totalSteps: stepState.stepIndex + 1,
          progressMessage:
            shouldReportStepProgress(stepState.stepIndex + 1) ?
              `Play progress: ${stepState.stepIndex + 1} steps completed`
            : prev.progressMessage,
          ...calcStepRates(stepState.stepIndex + 1, now),
        }));
        if (playingRef.current) {
          requestAnimationFrame(tick);
        }
      } catch {
        playingRef.current = false;
        const elapsed = runStartedAtRef.current != null ? Math.max(1, Math.floor((Date.now() - runStartedAtRef.current) / 1000)) : 0;
        setState((prev) => ({ ...prev, status: "completed", progressMessage: `Play completed in ${elapsed}s.` }));
      }
    };
    requestAnimationFrame(tick);
  }, [shouldReportStepProgress]);

  const pause = useCallback(() => {
    playingRef.current = false;
    setState((prev) => ({ ...prev, status: "paused", progressMessage: "Playback paused." }));
    if (sessionIdRef.current) {
      simulationCommand(sessionIdRef.current, "pause").catch(() => {});
    }
  }, []);

  const stop = useCallback(() => {
    playingRef.current = false;
    if (sessionIdRef.current) {
      simulationCommand(sessionIdRef.current, "stop").catch(() => {});
    }
    sessionIdRef.current = null;
    runStartedAtRef.current = null;
    lastProgressAtRef.current = 0;
    lastProgressStepRef.current = -1;
    stepTimestampsRef.current = [];
    setState({
      status: "idle",
      sessionId: null,
      currentStep: null,
      stepHistory: [],
      error: null,
      totalSteps: 0,
      progressMessage: null,
      averageStepsPerSec: 0,
      recentStepsPerSec: 0,
      controlEvents: [],
      progressEventCount: 0,
    });
  }, []);

  const reset = useCallback(() => {
    stop();
  }, [stop]);

  const seekTo = useCallback((stepIndex: number) => {
    setState((prev) => {
      if (stepIndex < 0 || stepIndex >= prev.stepHistory.length) return prev;
      return {
        ...prev,
        currentStep: prev.stepHistory[stepIndex],
      };
    });
  }, []);

  const getStateAtStep = useCallback(
    (stepIndex: number): StepState | null => {
      if (stepIndex < 0 || stepIndex >= state.stepHistory.length) return null;
      return state.stepHistory[stepIndex];
    },
    [state.stepHistory],
  );

  return {
    state,
    startSession,
    step,
    stepN,
    play,
    pause,
    stop,
    reset,
    seekTo,
    getStateAtStep,
  };
}
