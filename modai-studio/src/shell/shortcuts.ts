import type { MessageKey } from "../i18n";

export type SettingsTab = "general" | "layout" | "menu" | "tools" | "shortcuts";

export interface ShortcutEntry {
  keys: string;
  labelKey: MessageKey;
}

export const SHORTCUTS: ShortcutEntry[] = [
  { keys: "Ctrl+N", labelKey: "tipNew" },
  { keys: "Ctrl+O", labelKey: "tipOpen" },
  { keys: "Ctrl+S", labelKey: "tipSave" },
  { keys: "Ctrl+Enter", labelKey: "tipRun" },
  { keys: "Space", labelKey: "tipSimPauseResume" },
  { keys: ".", labelKey: "tipSimStep" },
  { keys: "Ctrl+.", labelKey: "tipSimStop" },
  { keys: "Ctrl+B", labelKey: "tipToggleSidebar" },
  { keys: "Ctrl+J", labelKey: "tipTogglePanel" },
  { keys: "Ctrl+Alt+B", labelKey: "tipToggleSecondary" },
  { keys: "Ctrl+Z", labelKey: "tipUndo" },
  { keys: "Ctrl+Y", labelKey: "tipRedo" },
  { keys: "Ctrl+C", labelKey: "tipCopy" },
  { keys: "Ctrl+V", labelKey: "tipPaste" },
  { keys: "Ctrl+R", labelKey: "tipRotate" },
  { keys: "Ctrl+F", labelKey: "tipFind" },
  { keys: "?", labelKey: "tipShortcuts" },
  { keys: "Esc", labelKey: "tipCloseDialog" },
];
