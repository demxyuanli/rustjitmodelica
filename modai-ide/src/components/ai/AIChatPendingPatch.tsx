import { useState } from "react";
import { t, tf } from "../../i18n";
import type { PendingPatch } from "../../hooks/useAI";

interface AIChatPendingPatchProps {
  pendingPatch: PendingPatch;
  currentFilePath?: string | null;
  aiLoading: boolean;
  onInsert: () => void;
}

export function AIChatPendingPatch({
  pendingPatch,
  currentFilePath,
  aiLoading,
  onInsert,
}: AIChatPendingPatchProps) {
  const [fileBarOpen, setFileBarOpen] = useState(true);
  if (!pendingPatch.newContent) return null;

  const fileCount = 1;

  return (
    <div className="agent-filebar mt-2">
      <div className="agent-filebar-header">
        <div className="agent-filebar-left">
          <span className="agent-filebar-count">
            {fileCount === 1
              ? tf("aiPendingFilesOne", { count: fileCount })
              : tf("aiPendingFilesOther", { count: fileCount })}
          </span>
          <span className="agent-filebar-range">
            {pendingPatch.filePath || currentFilePath || t("aiCurrentFile")}
            {pendingPatch.startLine != null && pendingPatch.endLine != null
              ? ` · ${tf("linesRange", { start: pendingPatch.startLine, end: pendingPatch.endLine })}`
              : ` · ${t("selection")}`}
          </span>
        </div>
        <div className="agent-filebar-right">
          {aiLoading && (
            <button
              type="button"
              className="agent-filebar-btn"
              disabled={!aiLoading}
            >
              {t("aiStop")}
            </button>
          )}
          <button
            type="button"
            className="agent-filebar-btn agent-filebar-primary"
            onClick={onInsert}
          >
            {t("aiReview")}
          </button>
          <button
            type="button"
            className="agent-filebar-toggle"
            onClick={() => setFileBarOpen((v) => !v)}
          >
            {fileBarOpen ? "▾" : "▴"}
          </button>
        </div>
      </div>
      {fileBarOpen && (
        <pre className="agent-filebar-body">
          {pendingPatch.newContent}
        </pre>
      )}
    </div>
  );
}
