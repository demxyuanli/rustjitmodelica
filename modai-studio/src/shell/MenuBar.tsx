import {
  ClipboardPaste,
  Copy,
  FileDown,
  FileJson,
  FilePlus2,
  FolderOpen,
  Package,
  Pause,
  Play,
  Redo2,
  Save,
  SlidersHorizontal,
  SkipForward,
  GitBranch,
  SquareStack,
  Undo2,
  Upload,
  FileCode2,
  Home,
} from "lucide-react";
import type { StudioSettings } from "../settings/studioSettings";
import { ExamplesMenu } from "./ExamplesMenu";
import { MenuDrop, MenuDropItem, MenuDropSep } from "./MenuDrop";
import { t } from "../i18n";

export interface MenuBarActions {
  onNew: () => void;
  onOpen: () => void;
  onOpenExample: (id: string) => void;
  onSave: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onSubsystem: () => void;
  onAddVariant: () => void;
  onExportCsv: () => void;
  onExportJson: () => void;
  onExportSvg: () => void;
  onExportPng: () => void;
  onEmitCoder: () => void;
  onEmitFmu: () => void;
  onImportFmu: () => void;
  onImportIde: () => void;
  onWrapMo: () => void;
  onImportPack: () => void;
  onExportPack: () => void;
  onPause: () => void;
  onResume: () => void;
  onStep: () => void;
  onConfig: () => void;
  onSettings: () => void;
  onWelcome: () => void;
  canExport: boolean;
  busy: boolean;
  runId: string | null;
  paused: boolean;
  canSubsystem: boolean;
  canAddVariant: boolean;
  projectLabel: string;
}

export function MenuBar({
  settings,
  actions,
  compact = false,
}: {
  settings: StudioSettings;
  actions: MenuBarActions;
  compact?: boolean;
}) {
  const vis = settings.menuVisible;
  const show = (id: keyof typeof vis) => vis[id] !== false;
  const fileOn =
    show("welcome") ||
    show("new") ||
    show("open") ||
    show("importIde") ||
    show("examples") ||
    show("save");
  const editOn =
    show("undo") ||
    show("redo") ||
    show("copy") ||
    show("paste") ||
    show("subsystem") ||
    show("addVariant");
  const exportOn =
    show("exportCsv") ||
    show("exportJson") ||
    show("exportSvg") ||
    show("exportPng") ||
    show("emitCoder") ||
    show("emitFmu") ||
    show("importFmu") ||
    show("wrapMo") ||
    show("importPack") ||
    show("exportPack");
  const simOn = show("pause") || show("resume") || show("step") || show("config");

  return (
    <div className={`menu-bar${compact ? " compact" : ""}`}>
      {fileOn ? (
        <MenuDrop labelKey="menuFile">
          {(close) => (
            <>
              {show("welcome") ? (
                <MenuDropItem
                  icon={Home}
                  labelKey="welcome"
                  onClick={() => {
                    close();
                    actions.onWelcome();
                  }}
                />
              ) : null}
              {show("new") ? (
                <MenuDropItem
                  icon={FilePlus2}
                  labelKey="new"
                  accel="Ctrl+N"
                  onClick={() => {
                    close();
                    actions.onNew();
                  }}
                />
              ) : null}
              {show("open") ? (
                <MenuDropItem
                  icon={FolderOpen}
                  labelKey="open"
                  accel="Ctrl+O"
                  onClick={() => {
                    close();
                    actions.onOpen();
                  }}
                />
              ) : null}
              {show("importIde") ? (
                <MenuDropItem
                  icon={FileJson}
                  labelKey="importIde"
                  onClick={() => {
                    close();
                    actions.onImportIde();
                  }}
                />
              ) : null}
              {show("save") ? (
                <MenuDropItem
                  icon={Save}
                  labelKey="save"
                  accel="Ctrl+S"
                  onClick={() => {
                    close();
                    actions.onSave();
                  }}
                />
              ) : null}
              {show("examples") ? (
                <>
                  <MenuDropSep />
                  <div className="menu-drop-examples">
                    <div className="menu-drop-caption">{t("examples")}</div>
                    <ExamplesMenu
                      onOpen={(id) => {
                        close();
                        actions.onOpenExample(id);
                      }}
                      showLabel
                      showTip={false}
                      embedded
                    />
                  </div>
                </>
              ) : null}
            </>
          )}
        </MenuDrop>
      ) : null}

      {editOn ? (
        <MenuDrop labelKey="menuEdit">
          {(close) => (
            <>
              {show("undo") ? (
                <MenuDropItem
                  icon={Undo2}
                  labelKey="undo"
                  accel="Ctrl+Z"
                  onClick={() => {
                    close();
                    actions.onUndo();
                  }}
                />
              ) : null}
              {show("redo") ? (
                <MenuDropItem
                  icon={Redo2}
                  labelKey="redo"
                  accel="Ctrl+Y"
                  onClick={() => {
                    close();
                    actions.onRedo();
                  }}
                />
              ) : null}
              <MenuDropSep />
              {show("copy") ? (
                <MenuDropItem
                  icon={Copy}
                  labelKey="copy"
                  accel="Ctrl+C"
                  onClick={() => {
                    close();
                    actions.onCopy();
                  }}
                />
              ) : null}
              {show("paste") ? (
                <MenuDropItem
                  icon={ClipboardPaste}
                  labelKey="paste"
                  accel="Ctrl+V"
                  onClick={() => {
                    close();
                    actions.onPaste();
                  }}
                />
              ) : null}
              <MenuDropSep />
              {show("subsystem") ? (
                <MenuDropItem
                  icon={SquareStack}
                  labelKey="subsystem"
                  disabled={!actions.canSubsystem}
                  onClick={() => {
                    close();
                    actions.onSubsystem();
                  }}
                />
              ) : null}
              {show("addVariant") ? (
                <MenuDropItem
                  icon={GitBranch}
                  labelKey="addVariant"
                  disabled={!actions.canAddVariant}
                  onClick={() => {
                    close();
                    actions.onAddVariant();
                  }}
                />
              ) : null}
            </>
          )}
        </MenuDrop>
      ) : null}

      {exportOn ? (
        <MenuDrop labelKey="menuExport">
          {(close) => (
            <>
              {show("exportCsv") ? (
                <MenuDropItem
                  icon={FileDown}
                  labelKey="exportCsv"
                  disabled={!actions.canExport}
                  onClick={() => {
                    close();
                    actions.onExportCsv();
                  }}
                />
              ) : null}
              {show("exportJson") ? (
                <MenuDropItem
                  icon={FileJson}
                  labelKey="exportJson"
                  disabled={!actions.canExport}
                  onClick={() => {
                    close();
                    actions.onExportJson();
                  }}
                />
              ) : null}
              {show("exportSvg") ? (
                <MenuDropItem
                  icon={FileCode2}
                  labelKey="exportSvg"
                  onClick={() => {
                    close();
                    actions.onExportSvg();
                  }}
                />
              ) : null}
              {show("exportPng") ? (
                <MenuDropItem
                  icon={FileDown}
                  labelKey="exportPng"
                  onClick={() => {
                    close();
                    void actions.onExportPng();
                  }}
                />
              ) : null}
              <MenuDropSep />
              {show("emitCoder") ? (
                <MenuDropItem
                  icon={Package}
                  labelKey="emitCoder"
                  disabled={actions.busy}
                  onClick={() => {
                    close();
                    actions.onEmitCoder();
                  }}
                />
              ) : null}
              {show("emitFmu") ? (
                <MenuDropItem
                  icon={Package}
                  labelKey="emitFmu"
                  disabled={actions.busy}
                  onClick={() => {
                    close();
                    actions.onEmitFmu();
                  }}
                />
              ) : null}
              {show("importFmu") ? (
                <MenuDropItem
                  icon={Upload}
                  labelKey="importFmu"
                  disabled={actions.busy}
                  onClick={() => {
                    close();
                    actions.onImportFmu();
                  }}
                />
              ) : null}
              {show("wrapMo") ? (
                <MenuDropItem
                  icon={FileCode2}
                  labelKey="wrapMo"
                  disabled={actions.busy}
                  onClick={() => {
                    close();
                    actions.onWrapMo();
                  }}
                />
              ) : null}
              {show("importPack") ? (
                <MenuDropItem
                  icon={Upload}
                  labelKey="importPack"
                  disabled={actions.busy}
                  onClick={() => {
                    close();
                    actions.onImportPack();
                  }}
                />
              ) : null}
              {show("exportPack") ? (
                <MenuDropItem
                  icon={Package}
                  labelKey="exportPack"
                  disabled={actions.busy}
                  onClick={() => {
                    close();
                    actions.onExportPack();
                  }}
                />
              ) : null}
            </>
          )}
        </MenuDrop>
      ) : null}

      {simOn ? (
        <MenuDrop labelKey="menuSim">
          {(close) => (
            <>
              {show("config") ? (
                <MenuDropItem
                  icon={SlidersHorizontal}
                  labelKey="config"
                  onClick={() => {
                    close();
                    actions.onConfig();
                  }}
                />
              ) : null}
              <MenuDropSep />
              {show("pause") ? (
                <MenuDropItem
                  icon={Pause}
                  labelKey="pause"
                  accel="Space"
                  disabled={!actions.busy || !actions.runId || actions.paused}
                  onClick={() => {
                    close();
                    actions.onPause();
                  }}
                />
              ) : null}
              {show("resume") ? (
                <MenuDropItem
                  icon={Play}
                  labelKey="resume"
                  accel="Space"
                  disabled={!actions.busy || !actions.runId || !actions.paused}
                  onClick={() => {
                    close();
                    actions.onResume();
                  }}
                />
              ) : null}
              {show("step") ? (
                <MenuDropItem
                  icon={SkipForward}
                  labelKey="step"
                  accel="."
                  disabled={!actions.busy || !actions.runId}
                  onClick={() => {
                    close();
                    actions.onStep();
                  }}
                />
              ) : null}
            </>
          )}
        </MenuDrop>
      ) : null}

      {!compact ? (
        <div className="menu-group menu-group-end">
          <span className="status-pill">{actions.projectLabel}</span>
        </div>
      ) : null}
    </div>
  );
}
