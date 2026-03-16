import { useState } from "react";
import { t } from "../i18n";
import { FileIcon } from "./FileIcon";
import { ContextMenu } from "./ContextMenu";

export interface EditorTab {
  id: string;
  path: string;
  dirty: boolean;
  projectPath?: string | null;
  readOnly?: boolean;
  modelName?: string;
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
  const [menuVisible, setMenuVisible] = useState(false);
  const [menuPosition, setMenuPosition] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const [menuIndex, setMenuIndex] = useState<number | null>(null);

  if (tabs.length === 0) return null;

  return (
    <div className="panel-header-min-height flex-1 min-w-0 flex items-center gap-0 overflow-x-auto scroll-vscode">
      {tabs.map((tab, i) => {
        const isActive = i === activeIndex;
        const label = tabLabel(tab.path);
        return (
          <div
            key={tab.path + String(i)}
            className={`flex items-center gap-[var(--toolbar-gap)] shrink-0 px-2 py-1.5 text-xs max-w-[160px] min-w-0 group ${
              isActive
                ? "bg-[var(--surface)] text-[var(--text)]"
                : "bg-[var(--surface-alt)] text-[var(--text-muted)] hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
            }`}
            onContextMenu={(event) => {
              event.preventDefault();
              setMenuPosition({ x: event.clientX, y: event.clientY });
              setMenuIndex(i);
              setMenuVisible(true);
            }}
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
      <ContextMenu
        visible={menuVisible}
        x={menuPosition.x}
        y={menuPosition.y}
        onClose={() => setMenuVisible(false)}
        items={
          menuIndex != null
            ? [
                {
                  id: "close",
                  label: t("closeTab"),
                  onClick: () => onCloseTab(menuIndex),
                },
                {
                  id: "close-others",
                  label: t("closeOthers") ?? "Close others",
                  onClick: () => {
                    for (let index = tabs.length - 1; index >= 0; index -= 1) {
                      if (index !== menuIndex) {
                        onCloseTab(index);
                      }
                    }
                  },
                },
                {
                  id: "close-right",
                  label: t("contextCloseTabsRight"),
                  onClick: () => {
                    for (let index = tabs.length - 1; index > menuIndex; index -= 1) {
                      onCloseTab(index);
                    }
                  },
                },
                {
                  id: "close-saved",
                  label: t("contextCloseSaved"),
                  onClick: () => {
                    for (let index = tabs.length - 1; index >= 0; index -= 1) {
                      if (index === menuIndex) continue;
                      const tab = tabs[index];
                      if (!tab.dirty) {
                        onCloseTab(index);
                      }
                    }
                  },
                },
                {
                  id: "copy-path",
                  label: t("contextCopyRelativePath"),
                  onClick: () => {
                    const tab = tabs[menuIndex];
                    const path = (tab.projectPath ?? tab.path) || "";
                    void navigator.clipboard.writeText(path);
                  },
                },
              ]
            : []
        }
      />
    </div>
  );
}
