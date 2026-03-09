import { t } from "../i18n";
import { FileIcon } from "./FileIcon";

export interface EditorTab {
  path: string;
  dirty: boolean;
}

interface EditorTabBarProps {
  tabs: EditorTab[];
  activeIndex: number;
  onSelectTab: (index: number) => void;
  onCloseTab: (index: number) => void;
}

function tabLabel(path: string): string {
  return path.replace(/^.*[/\\]/, "") || path || "?";
}

export function EditorTabBar({
  tabs,
  activeIndex,
  onSelectTab,
  onCloseTab,
}: EditorTabBarProps) {
  if (tabs.length === 0) return null;

  return (
    <div className="shrink-0 flex items-center gap-0 border-b border-border bg-surface-alt overflow-x-auto scroll-vscode">
      {tabs.map((tab, i) => {
        const isActive = i === activeIndex;
        const label = tabLabel(tab.path);
        return (
          <div
            key={tab.path + String(i)}
            className={`flex items-center gap-1 shrink-0 px-2 py-1.5 text-xs border-r border-border max-w-[160px] min-w-0 group ${
              isActive ? "bg-surface text-[var(--text)]" : "text-[var(--text-muted)] hover:bg-white/5 hover:text-[var(--text)]"
            }`}
          >
            <FileIcon name={label} size={14} />
            <button
              type="button"
              className="truncate flex-1 min-w-0 text-left"
              onClick={() => onSelectTab(i)}
              title={tab.path}
            >
              {label}
            </button>
            {tab.dirty && (
              <span className="shrink-0 w-1.5 h-1.5 rounded-full bg-amber-400" title={t("unsavedChanges")} aria-hidden />
            )}
            <button
              type="button"
              className="shrink-0 w-4 h-4 flex items-center justify-center rounded text-[var(--text-muted)] hover:bg-white/10 hover:text-[var(--text)] opacity-0 group-hover:opacity-100"
              onClick={(e) => {
                e.stopPropagation();
                onCloseTab(i);
              }}
              title={t("closeTab")}
              aria-label={t("closeTab")}
            >
              ×
            </button>
          </div>
        );
      })}
    </div>
  );
}
