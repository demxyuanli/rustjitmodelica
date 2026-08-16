import { useCallback, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Settings, Square, X } from "lucide-react";
import type { StudioSettings } from "../settings/studioSettings";
import { t } from "../i18n";
import { MenuBar, type MenuBarActions } from "./MenuBar";

export function TitleBar({
  settings,
  menuActions,
  title,
  onOpenSettings,
  onRequestClose,
}: {
  settings: StudioSettings;
  menuActions: MenuBarActions;
  title: string;
  onOpenSettings: () => void;
  onRequestClose?: () => boolean;
}) {
  const [maximized, setMaximized] = useState(false);

  const refreshMaximized = useCallback(async () => {
    try {
      setMaximized(await getCurrentWindow().isMaximized());
    } catch {
      /* browser preview */
    }
  }, []);

  useEffect(() => {
    void refreshMaximized();
  }, [refreshMaximized]);

  const minimize = async () => {
    try {
      await getCurrentWindow().minimize();
    } catch {
      /* ignore */
    }
  };

  const toggleMaximize = async () => {
    try {
      await getCurrentWindow().toggleMaximize();
      await refreshMaximized();
    } catch {
      /* ignore */
    }
  };

  const close = async () => {
    if (onRequestClose && !onRequestClose()) return;
    try {
      await getCurrentWindow().destroy();
    } catch {
      try {
        await getCurrentWindow().close();
      } catch {
        /* browser preview */
      }
    }
  };

  return (
    <header className="title-bar">
      <div className="title-bar-brand" data-tauri-drag-region>
        <span className="title-bar-app">{t("appTitle")}</span>
      </div>
      <div className="title-bar-menus">
        <MenuBar settings={settings} actions={menuActions} compact />
      </div>
      <div
        className="title-bar-center"
        data-tauri-drag-region
        onDoubleClick={() => void toggleMaximize()}
      >
        <span className="title-bar-title">{title}</span>
      </div>
      <div className="title-bar-actions">
        <button
          type="button"
          className="title-win-btn"
          title={t("settings")}
          aria-label={t("settings")}
          onClick={onOpenSettings}
        >
          <Settings size={14} strokeWidth={1.75} />
        </button>
        <button
          type="button"
          className="title-win-btn"
          title={t("windowMinimize")}
          aria-label={t("windowMinimize")}
          onClick={() => void minimize()}
        >
          <Minus size={14} strokeWidth={1.75} />
        </button>
        <button
          type="button"
          className="title-win-btn"
          title={maximized ? t("windowRestore") : t("windowMaximize")}
          aria-label={maximized ? t("windowRestore") : t("windowMaximize")}
          onClick={() => void toggleMaximize()}
        >
          <Square size={12} strokeWidth={1.75} />
        </button>
        <button
          type="button"
          className="title-win-btn title-win-close"
          title={t("windowClose")}
          aria-label={t("windowClose")}
          onClick={() => void close()}
        >
          <X size={14} strokeWidth={1.75} />
        </button>
      </div>
    </header>
  );
}
