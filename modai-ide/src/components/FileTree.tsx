import { t } from "../i18n";

interface FileTreeProps {
  projectDir: string | null;
  moFiles: string[];
  onOpenProject: () => void;
  onOpenFile: (relativePath: string) => void;
  lang: "en" | "zh";
  onToggleLang: () => void;
}

export function FileTree({
  projectDir,
  moFiles,
  onOpenProject,
  onOpenFile,
  lang,
  onToggleLang,
}: FileTreeProps) {
  return (
    <aside className="w-[240px] shrink-0 border-r border-border bg-surface-alt p-2 overflow-auto rounded-r-lg">
      <div className="flex items-center justify-between mb-2">
        <span className="text-sm font-medium text-[var(--text-muted)]">{t("project")}</span>
        <button type="button" onClick={onToggleLang} className="text-xs text-[var(--text-muted)] hover:text-[var(--text)]">
          {lang === "en" ? "\u4e2d\u6587" : "EN"}
        </button>
      </div>
      <button type="button" onClick={onOpenProject} className="w-full px-2 py-1.5 text-left text-sm rounded bg-primary/20 hover:bg-primary/30 text-primary mb-2">
        {t("openProject")}
      </button>
      {projectDir ? (
        <ul className="text-xs space-y-0.5 overflow-auto max-h-[calc(100vh-120px)]">
          {moFiles.map((f) => (
            <li key={f}>
              <button type="button" className="w-full text-left px-2 py-1 rounded hover:bg-white/10 truncate" onClick={() => onOpenFile(f)} title={f}>
                {f.split(/[/\\]/).pop() ?? f}
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <div className="text-xs text-[var(--text-muted)]">{t("noProjectOpen")}</div>
      )}
    </aside>
  );
}
