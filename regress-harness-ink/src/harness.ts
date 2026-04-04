import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

/**
 * Resolve path to regress-harness executable.
 * Override with env REGRESS_HARNESS_EXE.
 * Walks upward from cwd (e.g. regress-harness-ink/) to find workspace target/release.
 */
export function resolveHarnessExe(startDir: string): string | null {
  const env = process.env.REGRESS_HARNESS_EXE?.trim();
  if (env && existsSync(env)) {
    return env;
  }
  const isWin = process.platform === "win32";
  const name = isWin ? "regress-harness.exe" : "regress-harness";

  let dir = resolve(startDir);
  for (let depth = 0; depth < 32; depth++) {
    const candidates = [
      join(dir, "target", "release", name),
      join(dir, "target", "debug", name),
    ];
    for (const c of candidates) {
      if (existsSync(c)) {
        return c;
      }
    }
    const parent = dirname(dir);
    if (parent === dir) {
      break;
    }
    dir = parent;
  }
  return null;
}
