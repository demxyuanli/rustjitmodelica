import { t } from "../i18n";

interface EmptyEditorStateProps {
  hasProject: boolean;
  isSecondGroup?: boolean;
}

export function EmptyEditorState({ hasProject, isSecondGroup }: EmptyEditorStateProps) {
  const message = !hasProject
    ? t("editorEmptyNoProject")
    : t("editorEmptyOpenFile");

  return (
    <div className="editor-empty-state flex-1 min-h-0 flex flex-col items-center justify-center p-6 text-center">
      <p className="text-sm text-[var(--text-muted)] max-w-[280px]">
        {message}
      </p>
      {isSecondGroup && hasProject && (
        <p className="text-xs text-[var(--text-muted)] mt-2 opacity-80">
          {t("closeSplit")}
        </p>
      )}
    </div>
  );
}
