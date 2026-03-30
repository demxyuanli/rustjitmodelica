export type PatchAgentMode = "explain" | "edit-selection" | "edit-file" | "generate" | "iterate";

export interface ParsedPendingPatch {
  filePath: string | null;
  startLine: number | null;
  endLine: number | null;
  newContent: string | null;
}

export function buildIterateAssistantMessage(target: string, diff: string): string {
  return [
    `Prepared compiler patch for \`${target}\`. Review the diff and run the sandbox actions below.`,
    "",
    "```diff",
    diff,
    "```",
  ].join("\n");
}

export function parsePatchFromResponse(
  result: string,
  agentMode: PatchAgentMode,
  activeFilePath: string | null,
): ParsedPendingPatch | null {
  if (agentMode === "edit-selection") {
    const trimmed = result.trim();
    if (!trimmed) return null;
    return {
      filePath: activeFilePath,
      startLine: null,
      endLine: null,
      newContent: trimmed,
    };
  }
  if (agentMode === "edit-file") {
    const m = result.match(/```json\s*([\s\S]*?)```/i);
    if (!m) return null;
    try {
      const obj = JSON.parse(m[1].trim()) as {
        patches?: Array<{ filePath?: string; startLine?: number; endLine?: number; newContent?: string }>;
      };
      const list = obj.patches;
      if (!Array.isArray(list) || list.length === 0) return null;
      const patch = list.find(
        (p) =>
          !p.filePath ||
          p.filePath === activeFilePath ||
          (activeFilePath && activeFilePath.endsWith(p.filePath ?? "")),
      );
      if (!patch) return null;
      return {
        filePath: patch.filePath ?? activeFilePath,
        startLine: patch.startLine ?? null,
        endLine: patch.endLine ?? null,
        newContent: patch.newContent ?? null,
      };
    } catch {
      return null;
    }
  }
  return null;
}
