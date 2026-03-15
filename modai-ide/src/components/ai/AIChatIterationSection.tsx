import { useState } from "react";
import { t } from "../../i18n";
import { IterateActions } from "../IterateActions";
import { IterateHistory } from "../IterateHistory";
import type { IterationRecord, IterationRunResult } from "../../api/tauri";

interface AIChatIterationSectionProps {
  iterationDiff?: string | null;
  iterationRunResult?: IterationRunResult | null;
  iterationHistory: IterationRecord[];
  onRunIteration?: (quick: boolean) => Promise<unknown>;
  onAdoptIteration?: () => Promise<unknown>;
  onCommitIteration?: (message?: string) => Promise<unknown>;
  onReuseIteration?: (record: IterationRecord) => Promise<unknown>;
}

export function AIChatIterationSection({
  iterationDiff,
  iterationRunResult,
  iterationHistory,
  onRunIteration,
  onAdoptIteration,
  onCommitIteration,
  onReuseIteration,
}: AIChatIterationSectionProps) {
  const [iterationRunLoading, setIterationRunLoading] = useState(false);
  const [iterationAdoptLoading, setIterationAdoptLoading] = useState(false);
  const [iterationCommitLoading, setIterationCommitLoading] = useState(false);
  const [iterationCommitMessage, setIterationCommitMessage] = useState("");

  const canRunFull = !!iterationRunResult?.quick_run && !!iterationRunResult?.success;
  const canAdopt = !!iterationRunResult?.success && !!iterationDiff;
  const canCommit = !!iterationRunResult?.success && !iterationDiff;

  if (!iterationDiff && !iterationRunResult && iterationHistory.length === 0) return null;

  return (
    <div className="mt-2 space-y-2">
      {(iterationDiff || iterationRunResult) && (
        <div className="rounded-lg border border-border bg-[var(--panel-muted-bg)] p-3">
          <div className="text-[11px] font-medium text-[var(--text)] mb-2">{t("compilerIteration")}</div>
          <IterateActions
            runLoading={iterationRunLoading}
            adoptLoading={iterationAdoptLoading}
            commitLoading={iterationCommitLoading}
            canRunFull={canRunFull}
            canAdopt={canAdopt}
            canCommit={canCommit}
            commitMessage={iterationCommitMessage}
            onCommitMessageChange={setIterationCommitMessage}
            onRunQuick={async () => {
              if (!onRunIteration) return;
              setIterationRunLoading(true);
              try {
                await onRunIteration(true);
              } finally {
                setIterationRunLoading(false);
              }
            }}
            onRunFull={async () => {
              if (!onRunIteration) return;
              setIterationRunLoading(true);
              try {
                await onRunIteration(false);
              } finally {
                setIterationRunLoading(false);
              }
            }}
            onAdopt={async () => {
              if (!onAdoptIteration) return;
              setIterationAdoptLoading(true);
              try {
                await onAdoptIteration();
              } finally {
                setIterationAdoptLoading(false);
              }
            }}
            onCommit={async () => {
              if (!onCommitIteration) return;
              setIterationCommitLoading(true);
              try {
                await onCommitIteration(iterationCommitMessage);
              } finally {
                setIterationCommitLoading(false);
              }
            }}
          />
        </div>
      )}

      {iterationHistory.length > 0 && onReuseIteration && (
        <IterateHistory
          history={iterationHistory}
          onReuseDiff={async (record) => {
            await onReuseIteration(record);
          }}
        />
      )}
    </div>
  );
}
