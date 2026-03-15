import { useMemo } from "react";
import { t } from "../../i18n";

interface AIChatStatusBarProps {
  currentFilePath?: string | null;
  currentSelectionText?: string | null;
  lastJitErrorText?: string | null;
}

export function AIChatStatusBar({
  currentFilePath,
  currentSelectionText,
  lastJitErrorText,
}: AIChatStatusBarProps) {
  const hasSelection = useMemo(() => {
    return !!(currentSelectionText ?? "").trim();
  }, [currentSelectionText]);

  if (!currentFilePath && !hasSelection && !lastJitErrorText) return null;

  return (
    <div className="agent-status-row mt-2">
      {currentFilePath && (
        <span className="truncate max-w-[45%]" title={currentFilePath}>
          <AppIconInline name="file" />
          {currentFilePath.split(/[\\/]/).pop()}
        </span>
      )}
      {hasSelection && (
        <span className="agent-status-badge">{t("selection")}</span>
      )}
      {lastJitErrorText && lastJitErrorText.trim() && (
        <span className="agent-status-error">{t("jitError")}</span>
      )}
    </div>
  );
}

function AppIconInline({ name }: { name: string }) {
  if (name === "file") {
    return (
      <svg
        width="10"
        height="10"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        className="inline-block mr-1 -mt-px"
      >
        <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z" />
        <polyline points="14 2 14 8 20 8" />
      </svg>
    );
  }
  return null;
}
