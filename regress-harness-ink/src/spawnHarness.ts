import { spawn, type ChildProcess } from "node:child_process";

export type HarnessSpec =
  | { kind: "cli"; argv: string[] }
  | { kind: "repl-exec"; command: string };

export type StreamSinks = {
  onStdout: (s: string) => void;
  onStderr: (s: string) => void;
};

export function startHarnessJob(
  exe: string,
  cwd: string,
  spec: HarnessSpec,
  sinks: StreamSinks,
  onExit: (code: number | null, signal: NodeJS.Signals | null) => void
): ChildProcess {
  const argv =
    spec.kind === "cli" ? spec.argv : ["repl-exec", "-c", spec.command];
  const cmdLine =
    spec.kind === "cli"
      ? `${exe} ${argv.map((a) => (/\s/.test(a) ? `"${a.replace(/"/g, '\\"')}"` : a)).join(" ")}`
      : `${exe} repl-exec -c <line>`;
  sinks.onStdout(`$ ${cmdLine}\n`);
  const child = spawn(exe, argv, {
    cwd,
    env: process.env,
    shell: false,
  });
  let finished = false;
  const finish = (code: number | null, signal: NodeJS.Signals | null) => {
    if (finished) {
      return;
    }
    finished = true;
    onExit(code, signal);
  };
  child.stdout?.on("data", (b: Buffer) => {
    sinks.onStdout(b.toString("utf8"));
  });
  child.stderr?.on("data", (b: Buffer) => {
    sinks.onStderr(b.toString("utf8"));
  });
  child.on("error", (err: NodeJS.ErrnoException) => {
    sinks.onStderr(`[spawn error] ${err.message}\n`);
    finish(null, null);
  });
  child.on("close", (code, signal) => {
    finish(code, signal);
  });
  return child;
}
