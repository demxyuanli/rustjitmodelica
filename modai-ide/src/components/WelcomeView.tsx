import { t } from "../i18n";
import { recentProjectDisplayName } from "../hooks/useRecentProjects";
import { AppIcon } from "./Icon";

interface WelcomeViewProps {
  onOpenProject: () => void;
  recentProjects: string[];
  onOpenRecentProject: (path: string) => void;
}

export function WelcomeView({
  onOpenProject,
  recentProjects,
  onOpenRecentProject,
}: WelcomeViewProps) {
  return (
    <div className="flex-1 min-h-0 flex flex-col items-center pt-14 px-6 bg-transparent">
      <div className="w-full max-w-[540px] flex flex-col gap-8">
        <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
          <button
            type="button"
            onClick={onOpenProject}
            className="flex flex-col items-center gap-2.5 p-5 rounded-lg border border-[var(--border)] bg-[var(--surface)] hover:bg-[var(--surface-hover)] text-[var(--text)] transition-colors"
          >
            <AppIcon name="explorer" className="w-8 h-8 text-[var(--text-muted)]" aria-hidden />
            <span className="text-sm">{t("openProject")}</span>
          </button>
          <div
            className="flex flex-col items-center gap-2.5 p-5 rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--text-muted)] cursor-not-allowed opacity-70"
            title={t("cloneRepo")}
          >
            <AppIcon name="sourceControl" className="w-8 h-8" aria-hidden />
            <span className="text-sm">{t("cloneRepo")}</span>
          </div>
          <div
            className="flex flex-col items-center gap-2.5 p-5 rounded-lg border border-[var(--border)] bg-[var(--surface)] text-[var(--text-muted)] cursor-not-allowed opacity-70"
            title={t("openRemote")}
          >
            <AppIcon name="link" className="w-8 h-8" aria-hidden />
            <span className="text-sm">{t("openRemote")}</span>
          </div>
        </div>

        {recentProjects.length > 0 && (
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between">
              <span className="text-sm text-[var(--text-muted)]">{t("recentProjects")}</span>
              <span className="text-xs text-[var(--text-muted)]">({recentProjects.length})</span>
            </div>
            <ul className="flex flex-col">
              {recentProjects.map((dir) => (
                <li key={dir}>
                  <button
                    type="button"
                    className="w-full flex items-center justify-between gap-4 py-2 px-0 text-left text-sm text-[var(--text)] hover:underline underline-offset-2 min-w-0"
                    onClick={() => onOpenRecentProject(dir)}
                  >
                    <span className="truncate">{recentProjectDisplayName(dir)}</span>
                    <span className="text-xs text-[var(--text-muted)] truncate shrink-0 max-w-[220px]" title={dir}>
                      {dir}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </div>
  );
}
