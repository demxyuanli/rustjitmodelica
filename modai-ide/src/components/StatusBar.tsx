import { t } from "../i18n";

export interface StatusBarProps {
  gitBranch: string | null;
  openFilePath: string | null;
  language: string;
  position: { lineNumber: number; column: number } | null;
  errorCount: number;
  warningCount: number;
  onBranchClick?: () => void;
}

function Item({
  children,
  onClick,
  title,
}: {
  children: React.ReactNode;
  onClick?: () => void;
  title?: string;
}) {
  const base = "status-bar-item px-2 h-full flex items-center border-r border-border last:border-r-0";
  const interactive = onClick ? "cursor-pointer hover:bg-white/10" : "";
  return (
    <span
      className={`${base} ${interactive}`}
      onClick={onClick}
      onKeyDown={onClick ? (e) => e.key === "Enter" && onClick() : undefined}
      role={onClick ? "button" : undefined}
      tabIndex={onClick ? 0 : undefined}
      title={title}
    >
      {children}
    </span>
  );
}

export function StatusBar({
  gitBranch,
  openFilePath,
  language,
  position,
  errorCount,
  warningCount,
  onBranchClick,
}: StatusBarProps) {
  return (
    <footer
      className="status-bar shrink-0 h-[22px] flex items-center justify-between border-t border-border bg-[var(--surface-alt)] text-[var(--text-muted)] text-xs select-none"
      role="status"
    >
      <div className="flex items-center h-full min-w-0">
        {gitBranch != null && gitBranch !== "" && (
          <Item onClick={onBranchClick} title={t("sourceControl")}>
            <span className="mr-1" aria-hidden>&#xeab8;</span>
            {gitBranch}
          </Item>
        )}
        <Item title={openFilePath ?? undefined}>
          {openFilePath ?? t("noFile")}
        </Item>
        <Item>{language}</Item>
        {position != null && (
          <Item title={`Line ${position.lineNumber}, Column ${position.column}`}>
            Ln {position.lineNumber}, Col {position.column}
          </Item>
        )}
      </div>
      <div className="flex items-center h-full shrink-0">
        {(errorCount > 0 || warningCount > 0) && (
          <>
            {errorCount > 0 && (
              <Item title={`${errorCount} Error(s)`}>
                <span className="status-bar-error mr-0.5" aria-hidden>&#x2716;</span>
                {errorCount}
              </Item>
            )}
            {warningCount > 0 && (
              <Item title={`${warningCount} Warning(s)`}>
                <span className="status-bar-warning mr-0.5" aria-hidden>&#x26A0;</span>
                {warningCount}
              </Item>
            )}
          </>
        )}
      </div>
    </footer>
  );
}
