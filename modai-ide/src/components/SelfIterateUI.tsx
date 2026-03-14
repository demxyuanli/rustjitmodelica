import { IterateChat } from "./IterateChat";

interface SelfIterateUIProps {
  targetPrefill?: string | null;
  onClearPrefill?: () => void;
  fullScreen?: boolean;
  repoRoot?: string | null;
  onDiffGenerated?: (diff: string) => void;
  onRunResult?: (result: unknown) => void;
  initialContextFiles?: string[];
  onViewIterationDiff?: (iterationId: number, unifiedDiff: string, title?: string) => void;
}

export function SelfIterateUI({
  targetPrefill,
  onClearPrefill,
  repoRoot,
  onDiffGenerated,
  onRunResult,
  initialContextFiles,
  onViewIterationDiff,
}: SelfIterateUIProps) {
  return (
    <IterateChat
      targetPrefill={targetPrefill}
      onClearPrefill={onClearPrefill}
      repoRoot={repoRoot}
      initialContextFiles={initialContextFiles}
      onDiffGenerated={onDiffGenerated}
      onRunResult={onRunResult}
      onViewIterationDiff={onViewIterationDiff}
    />
  );
}
