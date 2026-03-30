export const simulationInputClass =
  "w-14 bg-[var(--surface)] border border-border px-1 text-sm rounded text-[var(--text)]";

export function pathToModelName(relativePath: string | null | undefined): string {
  if (!relativePath) return "";
  const withoutExt = relativePath.replace(/\.mo$/i, "");
  return withoutExt.replace(/[/\\]/g, ".");
}

export function parseUnknownTypeFromError(message: string): string | null {
  const m = message.match(/Unknown type '([^']+)' for instance/);
  return m ? m[1] : null;
}
