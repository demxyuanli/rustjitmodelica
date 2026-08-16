import { useEffect, useState } from "react";
import { Check, Keyboard, LayoutDashboard, Menu, RotateCcw, Settings2, Wrench, X } from "lucide-react";
import {
  DEFAULT_LAYOUT,
  DEFAULT_SETTINGS,
  FONT_PRESETS,
  MENU_IDS,
  TOOL_IDS,
  clampLayout,
  type MenuItemId,
  type StudioSettings,
  type ThemeId,
  type ToolItemId,
} from "./studioSettings";
import { CLASSIC_THEMES } from "./themes";
import { t, type MessageKey } from "../i18n";
import { IconButton } from "../shell/IconButton";
import { SHORTCUTS, type SettingsTab } from "../shell/shortcuts";

const menuLabelKey: Record<MenuItemId, MessageKey> = {
  welcome: "welcome",
  new: "new",
  open: "open",
  examples: "examples",
  save: "save",
  undo: "undo",
  redo: "redo",
  copy: "copy",
  paste: "paste",
  subsystem: "subsystem",
  addVariant: "addVariant",
  exportCsv: "exportCsv",
  exportJson: "exportJson",
  exportSvg: "exportSvg",
  exportPng: "exportPng",
  emitCoder: "emitCoder",
  emitFmu: "emitFmu",
  importFmu: "importFmu",
  importIde: "importIde",
  wrapMo: "wrapMo",
  importPack: "importPack",
  exportPack: "exportPack",
  pause: "pause",
  resume: "resume",
  step: "step",
  config: "config",
  settings: "settings",
};

const toolLabelKey: Record<ToolItemId, MessageKey> = {
  save: "save",
  update: "update",
  run: "run",
  stop: "stop",
  pause: "pause",
  resume: "resume",
  step: "step",
  snap: "snap",
  alignL: "alignL",
  alignT: "alignT",
  alignH: "alignH",
  alignV: "alignV",
  reroute: "reroute",
  solver: "solver",
  stopTime: "stopTime",
  intervals: "intervals",
  config: "config",
  dashboard: "dashboard",
  fastRestart: "fastRestart",
  fullJitValidate: "fullJitValidate",
};

const TABS: { id: SettingsTab; labelKey: MessageKey; icon: typeof Settings2 }[] = [
  { id: "general", labelKey: "settingsTabGeneral", icon: Settings2 },
  { id: "layout", labelKey: "settingsTabLayout", icon: LayoutDashboard },
  { id: "menu", labelKey: "settingsTabMenu", icon: Menu },
  { id: "tools", labelKey: "settingsTabTools", icon: Wrench },
  { id: "shortcuts", labelKey: "settingsTabShortcuts", icon: Keyboard },
];

function ShortcutsPanel() {
  return (
    <section className="settings-section">
      <p className="settings-hint">{t("settingsShortcutsHint")}</p>
      <table className="shortcuts-table">
        <thead>
          <tr>
            <th>{t("shortcutKeys")}</th>
            <th>{t("shortcutAction")}</th>
          </tr>
        </thead>
        <tbody>
          {SHORTCUTS.map((s) => (
            <tr key={s.keys}>
              <td>
                <kbd className="shortcut-kbd">{s.keys}</kbd>
              </td>
              <td>{t(s.labelKey)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}

export function SettingsDialog({
  open,
  settings,
  onChange,
  onClose,
  tab,
  onTabChange,
}: {
  open: boolean;
  settings: StudioSettings;
  onChange: (next: StudioSettings) => void;
  onClose: () => void;
  tab: SettingsTab;
  onTabChange: (tab: SettingsTab) => void;
}) {
  const [localTab, setLocalTab] = useState<SettingsTab>(tab);

  useEffect(() => {
    if (open) setLocalTab(tab);
  }, [open, tab]);

  useEffect(() => {
    if (!open) return;
    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") {
        ev.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  const active = localTab;
  const setActive = (next: SettingsTab) => {
    setLocalTab(next);
    onTabChange(next);
  };

  const patch = (partial: Partial<StudioSettings>) => onChange({ ...settings, ...partial });
  const patchLayout = (partial: Partial<StudioSettings["layout"]>) =>
    patch({ layout: clampLayout({ ...settings.layout, ...partial }) });

  return (
    <div className="settings-backdrop" role="presentation" onClick={onClose}>
      <div
        className="settings-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={t("settingsTitle")}
        onClick={(ev) => ev.stopPropagation()}
      >
        <header className="settings-header">
          <h2>{t("settingsTitle")}</h2>
          <IconButton icon={X} labelKey="settingsClose" showLabel={false} onClick={onClose} />
        </header>

        <nav className="settings-tabs" aria-label={t("settingsTitle")}>
          {TABS.map((item) => (
            <IconButton
              key={item.id}
              icon={item.icon}
              labelKey={item.labelKey}
              active={active === item.id}
              onClick={() => setActive(item.id)}
            />
          ))}
        </nav>

        {active === "general" ? (
          <section className="settings-section">
            <label>
              {t("settingsLocale")}
              <select
                value={settings.locale}
                onChange={(ev) => patch({ locale: ev.target.value as "zh" | "en" })}
              >
                <option value="zh">{t("localeZh")}</option>
                <option value="en">{t("localeEn")}</option>
              </select>
            </label>
            <div className="settings-theme-block">
              <div className="settings-theme-label">{t("settingsTheme")}</div>
              <p className="settings-hint">{t("settingsThemeHint")}</p>
              <div className="theme-swatch-row" role="listbox" aria-label={t("settingsTheme")}>
                {CLASSIC_THEMES.map((theme) => {
                  const active = settings.themeId === theme.id;
                  return (
                    <button
                      key={theme.id}
                      type="button"
                      role="option"
                      aria-selected={active}
                      className={`theme-swatch${active ? " active" : ""}`}
                      onClick={() => patch({ themeId: theme.id as ThemeId })}
                      title={t(theme.labelKey)}
                    >
                      <span
                        className="theme-swatch-preview"
                        style={{
                          background: `linear-gradient(135deg, ${theme.tokens.bg} 0 45%, ${theme.tokens.panel} 45% 70%, ${theme.tokens.accent} 70% 100%)`,
                          borderColor: theme.tokens.border,
                        }}
                      />
                      <span className="theme-swatch-name">{t(theme.labelKey)}</span>
                    </button>
                  );
                })}
              </div>
            </div>
            <label>
              {t("settingsFont")}
              <select
                value={settings.fontFamily}
                onChange={(ev) => patch({ fontFamily: ev.target.value })}
              >
                {FONT_PRESETS.map((f) => (
                  <option key={f} value={f}>
                    {f.replace(/"/g, "")}
                  </option>
                ))}
              </select>
            </label>
            <label>
              {t("settingsFontSize")}
              <input
                type="number"
                min={11}
                max={18}
                value={settings.fontSizePx}
                onChange={(ev) => patch({ fontSizePx: Number(ev.target.value) || 13 })}
              />
            </label>
            <label>
              {t("settingsDensity")}
              <select
                value={settings.density}
                onChange={(ev) =>
                  patch({ density: ev.target.value as "comfortable" | "compact" })
                }
              >
                <option value="comfortable">{t("densityComfortable")}</option>
                <option value="compact">{t("densityCompact")}</option>
              </select>
            </label>
            <label className="settings-check">
              <input
                type="checkbox"
                checked={settings.showTips}
                onChange={(ev) => patch({ showTips: ev.target.checked })}
              />
              {t("settingsTips")}
            </label>
            <label className="settings-check">
              <input
                type="checkbox"
                checked={settings.edgeLabelsVisible}
                onChange={(ev) => patch({ edgeLabelsVisible: ev.target.checked })}
              />
              {t("settingsEdgeLabels")}
            </label>
            <label>
              {t("settingsEdgeLabelOffset")}
              <input
                type="number"
                min={-40}
                max={40}
                step={1}
                value={settings.edgeLabelOffsetY}
                onChange={(ev) =>
                  patch({
                    edgeLabelOffsetY: Math.max(
                      -40,
                      Math.min(40, Math.round(Number(ev.target.value) || 0)),
                    ),
                  })
                }
              />
            </label>
          </section>
        ) : null}

        {active === "layout" ? (
          <section className="settings-section">
            <p className="settings-hint">{t("settingsLayoutHint")}</p>
            <label>
              {t("settingsLeftWidth")}
              <input
                type="number"
                min={160}
                max={520}
                value={settings.layout.leftWidth}
                onChange={(ev) => patchLayout({ leftWidth: Number(ev.target.value) })}
              />
            </label>
            <label>
              {t("settingsRightWidth")}
              <input
                type="number"
                min={200}
                max={520}
                value={settings.layout.rightWidth}
                onChange={(ev) => patchLayout({ rightWidth: Number(ev.target.value) })}
              />
            </label>
            <label>
              {t("settingsLibraryPct")}
              <input
                type="number"
                min={20}
                max={80}
                value={settings.layout.libraryPct}
                onChange={(ev) => patchLayout({ libraryPct: Number(ev.target.value) })}
              />
            </label>
            <label>
              {t("settingsResultsHeight")}
              <input
                type="number"
                min={120}
                max={520}
                value={settings.layout.resultsHeight}
                onChange={(ev) => patchLayout({ resultsHeight: Number(ev.target.value) })}
              />
            </label>
          </section>
        ) : null}

        {active === "menu" ? (
          <section className="settings-section">
            <h3>{t("settingsMenu")}</h3>
            <div className="settings-grid">
              {MENU_IDS.map((id) => (
                <label key={id} className="settings-check">
                  <input
                    type="checkbox"
                    checked={settings.menuVisible[id] !== false}
                    onChange={(ev) =>
                      patch({
                        menuVisible: { ...settings.menuVisible, [id]: ev.target.checked },
                      })
                    }
                  />
                  {t(menuLabelKey[id])}
                </label>
              ))}
            </div>
          </section>
        ) : null}

        {active === "tools" ? (
          <section className="settings-section">
            <h3>{t("settingsTools")}</h3>
            <div className="settings-grid">
              {TOOL_IDS.map((id) => (
                <label key={id} className="settings-check">
                  <input
                    type="checkbox"
                    checked={settings.toolVisible[id] !== false}
                    onChange={(ev) =>
                      patch({
                        toolVisible: { ...settings.toolVisible, [id]: ev.target.checked },
                      })
                    }
                  />
                  {t(toolLabelKey[id])}
                </label>
              ))}
            </div>
          </section>
        ) : null}

        {active === "shortcuts" ? <ShortcutsPanel /> : null}

        <footer className="settings-footer">
          <IconButton
            icon={RotateCcw}
            labelKey="settingsReset"
            onClick={() =>
              onChange({
                ...DEFAULT_SETTINGS,
                menuVisible: { ...DEFAULT_SETTINGS.menuVisible },
                toolVisible: { ...DEFAULT_SETTINGS.toolVisible },
                layout: { ...DEFAULT_LAYOUT },
              })
            }
          />
          <IconButton icon={Check} labelKey="settingsClose" primary onClick={onClose} />
        </footer>
      </div>
    </div>
  );
}
