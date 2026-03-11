import { getCurrentWindow } from "@tauri-apps/api/window";
import { t } from "../i18n";
import { AppIcon } from "./Icon";
import { IconButton } from "./IconButton";

export type WorkspaceMode = "modelica" | "compiler-iterate";

interface TitlebarProps {
  workspaceMode: WorkspaceMode;
  onWorkspaceModeChange: (mode: WorkspaceMode) => void;
  modelName: string;
  showProjectMenu: boolean;
  setShowProjectMenu: (v: boolean) => void;
  setShowSettings: (v: boolean) => void;
  showLeftSidebar: boolean;
  setShowLeftSidebar: (v: boolean) => void;
  showRightPanel: boolean;
  setShowRightPanel: (v: boolean) => void;
  showBottomPanel: boolean;
  setShowBottomPanel: (v: boolean) => void;
  lang: "en" | "zh";
  onToggleLang: () => void;
  onOpenProject?: () => void;
}

export function Titlebar({
  workspaceMode,
  onWorkspaceModeChange,
  modelName,
  showProjectMenu,
  setShowProjectMenu,
  setShowSettings,
  showLeftSidebar,
  setShowLeftSidebar,
  showRightPanel,
  setShowRightPanel,
  showBottomPanel,
  setShowBottomPanel,
  lang,
  onToggleLang,
  onOpenProject,
}: TitlebarProps) {
  return (
    <header
      className="titlebar flex items-center h-9 shrink-0 bg-[#323233] dark:bg-[#323233] border-b border-border select-none gap-0 pr-0 text-sm"
      data-tauri-drag-region
    >
      <div className="flex items-center h-full pl-2 gap-2" data-tauri-drag-region>
        <div className="flex rounded overflow-hidden border border-gray-600">
          <IconButton
            icon={<AppIcon name="explorer" aria-hidden="true" />}
            variant="ghost"
            active={workspaceMode === "modelica"}
            className="titlebar-btn h-7 w-9"
            onClick={() => onWorkspaceModeChange("modelica")}
            title={t("workspaceModelica")}
            aria-label={t("workspaceModelica")}
          />
          <IconButton
            icon={<AppIcon name="sourceControl" aria-hidden="true" />}
            variant="ghost"
            active={workspaceMode === "compiler-iterate"}
            className="titlebar-btn h-7 w-9"
            onClick={() => onWorkspaceModeChange("compiler-iterate")}
            title={t("workspaceCompilerIterate")}
            aria-label={t("workspaceCompilerIterate")}
          />
        </div>
        <div className="relative">
          <button
            type="button"
            className="titlebar-btn h-full px-2 flex items-center gap-1 text-[#cccccc] hover:bg-white/10"
            onClick={(e) => {
              e.stopPropagation();
              setShowProjectMenu(!showProjectMenu);
            }}
            title={modelName || t("project")}
          >
            <AppIcon name="explorer" aria-hidden="true" className="w-3.5 h-3.5" />
            <span className="max-w-[120px] truncate">{modelName}</span>
            <svg width="10" height="10" viewBox="0 0 10 10" className="ml-0.5">
              <path fill="currentColor" d="M2 3l3 3 3-3H2z" />
            </svg>
          </button>
          {showProjectMenu && (
            <div className="absolute left-0 top-full mt-0 bg-[#252526] border border-gray-700 shadow-lg z-50 min-w-[200px] py-1 rounded">
              <button
                type="button"
                className="w-full text-left px-3 py-1.5 text-sm text-[#cccccc] hover:bg-white/10"
                onClick={() => { onOpenProject?.(); setShowProjectMenu(false); }}
              >
                {t("openProject")}
              </button>
              <div className="border-t border-gray-700 my-1" />
              <button
                type="button"
                className="w-full text-left px-3 py-1.5 text-sm text-[#cccccc] hover:bg-white/10"
                onClick={() => { setShowSettings(true); setShowProjectMenu(false); }}
              >
                {t("settings")}
              </button>
            </div>
          )}
        </div>
      </div>
      <div className="flex-1 flex items-center justify-end gap-1 px-2" data-tauri-drag-region>
        <IconButton
          icon={<AppIcon name="language" aria-hidden="true" />}
          size="xs"
          className="titlebar-btn px-2 h-6 text-[#cccccc] hover:bg-white/10"
          onClick={onToggleLang}
          title={lang === "en" ? "Switch language to Chinese" : "Switch language to English"}
          aria-label={lang === "en" ? "Switch language to Chinese" : "Switch language to English"}
        />
        <button type="button" className="titlebar-btn w-7 h-7 flex items-center justify-center text-[#cccccc] hover:bg-white/10" onClick={() => setShowSettings(true)} title={t("settings")}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5"><circle cx="12" cy="12" r="3" /><path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" /></svg>
        </button>
        <button type="button" className={`titlebar-btn w-7 h-7 flex items-center justify-center text-[#cccccc] hover:bg-white/10 ${showLeftSidebar ? "bg-white/5" : ""}`} onClick={() => setShowLeftSidebar(!showLeftSidebar)} title="Left sidebar">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5"><rect x="2" y="3" width="7" height="18" rx="0.5" /><rect x="11" y="3" width="11" height="18" rx="0.5" /></svg>
        </button>
        <button type="button" className={`titlebar-btn w-7 h-7 flex items-center justify-center text-[#cccccc] hover:bg-white/10 ${showRightPanel ? "bg-white/5" : ""}`} onClick={() => setShowRightPanel(!showRightPanel)} title="Right panel">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5"><rect x="2" y="3" width="11" height="18" rx="0.5" /><rect x="15" y="3" width="7" height="18" rx="0.5" /></svg>
        </button>
        <button type="button" className={`titlebar-btn w-7 h-7 flex items-center justify-center text-[#cccccc] hover:bg-white/10 ${showBottomPanel ? "bg-white/5" : ""}`} onClick={() => setShowBottomPanel(!showBottomPanel)} title="Bottom panel">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5"><rect x="2" y="3" width="20" height="12" rx="0.5" /><rect x="2" y="17" width="20" height="4" rx="0.5" /></svg>
        </button>
      </div>
      <div className="flex items-stretch h-full">
        <button type="button" className="titlebar-btn w-12 flex items-center justify-center text-[#cccccc] hover:bg-white/10" onClick={() => getCurrentWindow().minimize()} title="Minimize">
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1"><line x1="0" y1="5" x2="10" y2="5" /></svg>
        </button>
        <button type="button" className="titlebar-btn w-12 flex items-center justify-center text-[#cccccc] hover:bg-white/10" onClick={() => getCurrentWindow().toggleMaximize()}>
          <svg width="10" height="10" fill="none" stroke="currentColor" strokeWidth="1" viewBox="0 0 10 10"><rect x="0" y="0" width="9" height="9" /></svg>
        </button>
        <button type="button" className="titlebar-btn w-12 flex items-center justify-center text-[#cccccc] hover:bg-[#e81123] hover:text-white" onClick={() => getCurrentWindow().close()}>
          <svg width="10" height="10" viewBox="0 0 10 10"><path stroke="currentColor" strokeWidth="1" d="M0 0 L10 10 M10 0 L0 10" /></svg>
        </button>
      </div>
    </header>
  );
}
