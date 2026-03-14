import { t } from "../i18n";

interface IterateActionsProps {
  runLoading: boolean;
  adoptLoading: boolean;
  commitLoading: boolean;
  canRunFull: boolean;
  canAdopt: boolean;
  canCommit: boolean;
  commitMessage: string;
  onCommitMessageChange: (value: string) => void;
  onRunQuick: () => void;
  onRunFull: () => void;
  onAdopt: () => void;
  onCommit: () => void;
}

export function IterateActions({
  runLoading,
  adoptLoading,
  commitLoading,
  canRunFull,
  canAdopt,
  canCommit,
  commitMessage,
  onCommitMessageChange,
  onRunQuick,
  onRunFull,
  onAdopt,
  onCommit,
}: IterateActionsProps) {
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          onClick={onRunQuick}
          disabled={runLoading}
          className="px-3 py-1.5 rounded-lg bg-primary hover:bg-blue-600 text-white text-xs font-medium disabled:opacity-50"
        >
          {runLoading ? t("running") : t("runInSandbox")}
        </button>
        {canRunFull && (
          <button
            type="button"
            onClick={onRunFull}
            disabled={runLoading}
            className="px-3 py-1.5 rounded-lg border theme-button-secondary text-xs font-medium disabled:opacity-50"
          >
            {runLoading ? t("running") : t("runFullBuild")}
          </button>
        )}
        {canAdopt && (
          <button
            type="button"
            onClick={onAdopt}
            disabled={adoptLoading}
            className="px-3 py-1.5 rounded-lg bg-green-700 hover:bg-green-600 text-white text-xs font-medium disabled:opacity-50"
          >
            {adoptLoading ? t("running") : t("adoptToWorkspace")}
          </button>
        )}
      </div>

      {canCommit && (
        <div className="flex flex-wrap items-center gap-2">
          <input
            type="text"
            value={commitMessage}
            onChange={(event) => onCommitMessageChange(event.target.value)}
            placeholder={t("commitMessage")}
            className="min-w-[240px] flex-1 theme-input border px-3 py-2 text-xs rounded-lg"
          />
          <button
            type="button"
            onClick={onCommit}
            disabled={commitLoading}
            className="px-3 py-1.5 rounded-lg border theme-button-secondary text-xs font-medium disabled:opacity-50"
          >
            {commitLoading ? t("running") : t("commit")}
          </button>
        </div>
      )}
    </div>
  );
}
