import type { StudioProgressEvent } from "../api/studio";
import { t, tf, type MessageKey } from "../i18n";

const EXACT: Record<string, MessageKey> = {
  "Starting FMI2 CS cosimulation": "progFmuStart",
  "FMU cosimulation done": "progFmuDone",
  "Simulation started (Fast Restart enabled)": "progStartedFast",
  "Simulation started": "progStarted",
  "fmu cosim failed": "progFmuFailed",
  failed: "statusRunFailed",
};

const RUNNING_RE = /^t=([\d.]+)\/([\d.]+) step=(\d+) wall=(\d+)s$/;
const DONE_RE = /^Simulation done \((fastRestart|fullCompile)\)$/;

function fmtNum(n: number | null | undefined, digits = 4): string {
  if (n == null || Number.isNaN(n)) return "-";
  return n.toFixed(digits);
}

function modeLabel(mode: string): string {
  if (mode === "fastRestart") return t("modeFastRestart");
  if (mode === "fullCompile") return t("modeFullCompile");
  return t("modeRun");
}

/** Map known backend progress messages / phases to localized UI text. */
export function localizeProgress(ev: StudioProgressEvent): string {
  const byPhase = localizeByPhase(ev);
  if (byPhase) return byPhase;
  return localizeMessage(ev.message);
}

export function localizeMessage(message: string): string {
  const exact = EXACT[message];
  if (exact) return t(exact);

  const running = RUNNING_RE.exec(message);
  if (running) {
    return tf("progRunning", {
      t: running[1]!,
      tEnd: running[2]!,
      step: running[3]!,
      wall: running[4]!,
    });
  }

  const done = DONE_RE.exec(message);
  if (done) {
    return tf("progDone", { mode: modeLabel(done[1]!) });
  }

  return message;
}

function localizeByPhase(ev: StudioProgressEvent): string | null {
  switch (ev.phase) {
    case "emit":
      return t("progEmit");
    case "compile":
      return ev.message.toLowerCase().includes("fast")
        ? t("progCompileFast")
        : t("progCompile");
    case "started":
      return ev.message.includes("Fast Restart")
        ? t("progStartedFast")
        : t("progStarted");
    case "fmu_cosim":
      return t("progFmuStart");
    case "running":
      return tf("progRunning", {
        t: fmtNum(ev.simTime),
        tEnd: fmtNum(ev.tEnd),
        step: ev.step ?? 0,
        wall: extractWall(ev.message),
      });
    case "done":
      if (ev.message.includes("FMU") || ev.message.toLowerCase().includes("fmu")) {
        return t("progFmuDone");
      }
      {
        const m = DONE_RE.exec(ev.message);
        if (m) return tf("progDone", { mode: modeLabel(m[1]!) });
        if (ev.message.includes("fastRestart")) {
          return tf("progDone", { mode: t("modeFastRestart") });
        }
        if (ev.message.includes("fullCompile")) {
          return tf("progDone", { mode: t("modeFullCompile") });
        }
        return t("progDoneGeneric");
      }
    case "cancelled":
      return t("statusCancelled");
    case "error":
      return localizeMessage(ev.message);
    default:
      return null;
  }
}

function extractWall(message: string): string | number {
  const m = /wall=(\d+)s/.exec(message);
  return m?.[1] ?? "-";
}
