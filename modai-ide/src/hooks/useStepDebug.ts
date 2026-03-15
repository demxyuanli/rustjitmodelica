import { useState, useCallback, useRef } from "react";
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
  const [state, setState] = useState<StepDebugState>({
    status: "idle",
    sessionId: null,
    currentStep: null,
    stepHistory: [],
    error: null,
    totalSteps: 0,
  });

  const playingRef = useRef(false);
  const sessionIdRef = useRef<string | null>(null);

  const startSession = useCallback(
    async (code: string, modelName?: string, projectDir?: string | null) => {
      setState((prev) => ({ ...prev, status: "compiling", error: null, stepHistory: [], currentStep: null }));
      try {
        const sid = await startSimulationSession(code, modelName, projectDir);
        sessionIdRef.current = sid;
        setState((prev) => ({
          ...prev,
          status: "paused",
          sessionId: sid,
          error: null,
        }));
      } catch (err) {
        setState((prev) => ({
          ...prev,
          status: "error",
          error: String(err),
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
      setState((prev) => ({
        ...prev,
        status: "paused",
        currentStep: stepState,
        stepHistory: [...prev.stepHistory, stepState],
        totalSteps: stepState.stepIndex + 1,
      }));
    } catch (err) {
      const errMsg = String(err);
      if (errMsg.includes("completed")) {
        setState((prev) => ({ ...prev, status: "completed" }));
      } else {
        setState((prev) => ({ ...prev, status: "error", error: errMsg }));
      }
    }
  }, []);

  const stepN = useCallback(
    async (n: number) => {
      for (let i = 0; i < n; i++) {
        const sid = sessionIdRef.current;
        if (!sid) break;
        try {
          const stepState = await simulationStep(sid);
          setState((prev) => ({
            ...prev,
            status: "paused",
            currentStep: stepState,
            stepHistory: [...prev.stepHistory, stepState],
            totalSteps: stepState.stepIndex + 1,
          }));
        } catch {
          setState((prev) => ({ ...prev, status: "completed" }));
          break;
        }
      }
    },
    [],
  );

  const play = useCallback(() => {
    playingRef.current = true;
    setState((prev) => ({ ...prev, status: "running" }));
    const tick = async () => {
      if (!playingRef.current || !sessionIdRef.current) return;
      try {
        const stepState = await simulationStep(sessionIdRef.current);
        setState((prev) => ({
          ...prev,
          currentStep: stepState,
          stepHistory: [...prev.stepHistory, stepState],
          totalSteps: stepState.stepIndex + 1,
        }));
        if (playingRef.current) {
          requestAnimationFrame(tick);
        }
      } catch {
        playingRef.current = false;
        setState((prev) => ({ ...prev, status: "completed" }));
      }
    };
    requestAnimationFrame(tick);
  }, []);

  const pause = useCallback(() => {
    playingRef.current = false;
    setState((prev) => ({ ...prev, status: "paused" }));
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
    setState({
      status: "idle",
      sessionId: null,
      currentStep: null,
      stepHistory: [],
      error: null,
      totalSteps: 0,
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
