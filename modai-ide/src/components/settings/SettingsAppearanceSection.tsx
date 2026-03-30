import { t } from "../../i18n";
import { DiagramColorSchemePreview } from "../DiagramColorSchemePreview";
import {
  DEFAULT_CONNECTOR_KEYS,
  DIAGRAM_COLOR_SCHEMES,
  setSingleColorOverride,
  clearColorOverrides,
  hasColorOverrides,
  type DiagramColorScheme,
} from "../../utils/diagramColorSchemes";
import { SettingsRow } from "./settingsPrimitives";
import { CONNECTOR_LABEL_KEYS } from "./settingsDiagramConstants";

export interface SettingsAppearanceSectionProps {
  theme: "dark" | "light";
  onThemeChange: (theme: "dark" | "light") => void;
  fontUi: "chinese" | "code";
  onFontUiChange: (v: "chinese" | "code") => void;
  fontSizePercent: 90 | 100 | 110 | 120;
  onFontSizePercentChange: (v: 90 | 100 | 110 | 120) => void;
  uiColorScheme?: "default" | "classic";
  onUiColorSchemeChange?: (v: "default" | "classic") => void;
  panelHeaderHeight?: number;
  onPanelHeaderHeightChange?: (v: number) => void;
  toolbarBtnSize?: number;
  onToolbarBtnSizeChange?: (v: number) => void;
  toolbarGap?: number;
  onToolbarGapChange?: (v: number) => void;
  diagramSchemeId?: string;
  diagramScheme?: DiagramColorScheme;
  onDiagramSchemeChange?: (id: string) => void;
}

export function SettingsAppearanceSection({
  theme,
  onThemeChange,
  fontUi,
  onFontUiChange,
  fontSizePercent,
  onFontSizePercentChange,
  uiColorScheme,
  onUiColorSchemeChange,
  panelHeaderHeight,
  onPanelHeaderHeightChange,
  toolbarBtnSize,
  onToolbarBtnSizeChange,
  toolbarGap,
  onToolbarGapChange,
  diagramSchemeId,
  diagramScheme,
  onDiagramSchemeChange,
}: SettingsAppearanceSectionProps) {
  return (
    <section id="settings-group-appearance">
      <h3 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-1">{t("settingsSectionAppearance")}</h3>
      <SettingsRow title={t("settingsAppearance")} description={t("settingsAppearanceDesc")}>
        <div className="flex rounded-md overflow-hidden border border-border">
          <button type="button" className={`px-3 py-1.5 text-xs ${theme === "dark" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onThemeChange("dark")}>{t("themeDark")}</button>
          <button type="button" className={`px-3 py-1.5 text-xs ${theme === "light" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onThemeChange("light")}>{t("themeLight")}</button>
        </div>
      </SettingsRow>
      <SettingsRow title={t("settingsFontUi")} description={t("settingsFontUiDesc")}>
        <div className="flex rounded-md overflow-hidden border border-border">
          <button type="button" className={`px-3 py-1.5 text-xs ${fontUi === "chinese" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onFontUiChange("chinese")}>{t("fontUiChinese")}</button>
          <button type="button" className={`px-3 py-1.5 text-xs ${fontUi === "code" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onFontUiChange("code")}>{t("fontUiCode")}</button>
        </div>
      </SettingsRow>
      <SettingsRow title={t("settingsFontSize")} description={t("settingsFontSizeDesc")}>
        <div className="flex rounded-md overflow-hidden border border-border">
          <button type="button" className={`px-3 py-1.5 text-xs ${fontSizePercent === 90 ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onFontSizePercentChange(90)}>{t("fontSize90")}</button>
          <button type="button" className={`px-3 py-1.5 text-xs ${fontSizePercent === 100 ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onFontSizePercentChange(100)}>{t("fontSize100")}</button>
          <button type="button" className={`px-3 py-1.5 text-xs ${fontSizePercent === 110 ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onFontSizePercentChange(110)}>{t("fontSize110")}</button>
          <button type="button" className={`px-3 py-1.5 text-xs ${fontSizePercent === 120 ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onFontSizePercentChange(120)}>{t("fontSize120")}</button>
        </div>
      </SettingsRow>
      {onUiColorSchemeChange != null && uiColorScheme != null && (
        <SettingsRow title={t("settingsUiColorScheme")} description={t("settingsUiColorSchemeDesc")}>
          <div className="flex rounded-md overflow-hidden border border-border">
            <button type="button" className={`px-3 py-1.5 text-xs ${uiColorScheme === "default" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onUiColorSchemeChange("default")}>{t("uiColorDefault")}</button>
            <button type="button" className={`px-3 py-1.5 text-xs ${uiColorScheme === "classic" ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onUiColorSchemeChange("classic")}>{t("uiColorClassic")}</button>
          </div>
        </SettingsRow>
      )}
      {onPanelHeaderHeightChange != null && panelHeaderHeight != null && (
        <SettingsRow title={t("settingsLayoutPanelHeaderHeight")} description={t("settingsLayoutPanelHeaderHeightDesc")}>
          <div className="flex rounded-md overflow-hidden border border-border">
            {[28, 32, 36].map((px) => (
              <button key={px} type="button" className={`px-3 py-1.5 text-xs ${panelHeaderHeight === px ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onPanelHeaderHeightChange(px)}>{px}px</button>
            ))}
          </div>
        </SettingsRow>
      )}
      {onToolbarBtnSizeChange != null && toolbarBtnSize != null && (
        <SettingsRow title={t("settingsLayoutToolbarBtnSize")} description={t("settingsLayoutToolbarBtnSizeDesc")}>
          <div className="flex rounded-md overflow-hidden border border-border">
            {[24, 26, 28].map((px) => (
              <button key={px} type="button" className={`px-3 py-1.5 text-xs ${toolbarBtnSize === px ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onToolbarBtnSizeChange(px)}>{px}px</button>
            ))}
          </div>
        </SettingsRow>
      )}
      {onToolbarGapChange != null && toolbarGap != null && (
        <SettingsRow title={t("settingsLayoutToolbarGap")} description={t("settingsLayoutToolbarGapDesc")}>
          <div className="flex rounded-md overflow-hidden border border-border">
            {[6, 8, 10].map((px) => (
              <button key={px} type="button" className={`px-3 py-1.5 text-xs ${toolbarGap === px ? "bg-primary text-white" : "bg-[var(--surface)] text-[var(--text-muted)] hover:bg-white/5"}`} onClick={() => onToolbarGapChange(px)}>{px}px</button>
            ))}
          </div>
        </SettingsRow>
      )}
      {onDiagramSchemeChange != null && diagramSchemeId != null && (
        <div className="flex flex-col gap-2 py-4 border-b border-[var(--border)] last:border-b-0">
          <div className="min-w-0">
            <div className="flex items-center gap-1.5">
              <span className="text-sm font-medium text-[var(--text)]">{t("settingsDiagramColors")}</span>
              <span className="text-[var(--text-muted)] opacity-70" title={t("settingsDiagramColorsDesc")} aria-hidden="true">&#9432;</span>
            </div>
            <p className="text-xs text-[var(--text-muted)] mt-1">{t("settingsDiagramColorsDesc")}</p>
          </div>
          <div className="flex flex-wrap gap-3 justify-start">
            {DIAGRAM_COLOR_SCHEMES.map((scheme) => (
              <button
                key={scheme.id}
                type="button"
                onClick={() => onDiagramSchemeChange(scheme.id)}
                className={`rounded-lg border-2 p-1.5 transition-colors flex-shrink-0 ${diagramSchemeId === scheme.id ? "border-[var(--primary)] ring-2 ring-[var(--primary)]/30 bg-[var(--primary)]/5" : "border-[var(--border)] hover:border-[var(--text-muted)] hover:bg-white/5"}`}
              >
                <DiagramColorSchemePreview scheme={scheme} compact />
              </button>
            ))}
          </div>
          {diagramSchemeId && diagramScheme && (
            <div className="mt-4 pt-4 border-t border-[var(--border)] space-y-4">
              <div className="flex items-center justify-between gap-2">
                <span className="text-sm font-medium text-[var(--text)]">{t("settingsDiagramEditColors")}</span>
                {hasColorOverrides(diagramSchemeId) && (
                  <button
                    type="button"
                    onClick={() => clearColorOverrides(diagramSchemeId)}
                    className="px-2.5 py-1 text-xs rounded border border-[var(--border)] bg-[var(--surface)] text-[var(--text-muted)] hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
                  >
                    {t("settingsDiagramResetScheme")}
                  </button>
                )}
              </div>
              <div>
                <div className="text-xs font-medium text-[var(--text-muted)] mb-2">{t("settingsDiagramConnectorColors")}</div>
                <div className="flex flex-wrap gap-3">
                  {DEFAULT_CONNECTOR_KEYS.map((key) => (
                    <div key={key} className="flex items-center gap-2">
                      <label className="text-xs text-[var(--text)] whitespace-nowrap">{t(CONNECTOR_LABEL_KEYS[key] ?? key)}</label>
                      <input
                        type="color"
                        value={diagramScheme.connectorColors[key] ?? "#888"}
                        onChange={(e) => setSingleColorOverride(diagramSchemeId, "connectorColors", key, e.target.value)}
                        className="w-8 h-8 rounded border border-[var(--border)] cursor-pointer bg-[var(--surface)]"
                        title={t(CONNECTOR_LABEL_KEYS[key] ?? key)}
                      />
                    </div>
                  ))}
                </div>
              </div>
              <div>
                <div className="text-xs font-medium text-[var(--text-muted)] mb-2">{t("settingsDiagramChartPaletteLight")}</div>
                <div className="flex flex-wrap gap-2">
                  {(diagramScheme.chartPaletteLight ?? []).map((color, i) => (
                    <div key={`light-${i}`} className="flex items-center gap-1">
                      <span className="text-[10px] text-[var(--text-muted)] w-3">{i + 1}</span>
                      <input
                        type="color"
                        value={color}
                        onChange={(e) => setSingleColorOverride(diagramSchemeId, "chartPaletteLight", i, e.target.value)}
                        className="w-8 h-8 rounded border border-[var(--border)] cursor-pointer bg-[var(--surface)]"
                        title={`${t("settingsDiagramChartPaletteLight")} ${i + 1}`}
                      />
                    </div>
                  ))}
                </div>
              </div>
              <div>
                <div className="text-xs font-medium text-[var(--text-muted)] mb-2">{t("settingsDiagramChartPaletteDark")}</div>
                <div className="flex flex-wrap gap-2">
                  {(diagramScheme.chartPaletteDark ?? []).map((color, i) => (
                    <div key={`dark-${i}`} className="flex items-center gap-1">
                      <span className="text-[10px] text-[var(--text-muted)] w-3">{i + 1}</span>
                      <input
                        type="color"
                        value={color}
                        onChange={(e) => setSingleColorOverride(diagramSchemeId, "chartPaletteDark", i, e.target.value)}
                        className="w-8 h-8 rounded border border-[var(--border)] cursor-pointer bg-[var(--surface)]"
                        title={`${t("settingsDiagramChartPaletteDark")} ${i + 1}`}
                      />
                    </div>
                  ))}
                </div>
              </div>
              <div>
                <div className="text-xs font-medium text-[var(--text-muted)] mb-2">{t("settingsDiagramPrimaryColor")}</div>
                <div className="flex items-center gap-2">
                  <input
                    type="color"
                    value={diagramScheme.diagramPrimary ?? "#3b82f6"}
                    onChange={(e) => setSingleColorOverride(diagramSchemeId, "diagramPrimary", 0, e.target.value)}
                    className="w-8 h-8 rounded border border-[var(--border)] cursor-pointer bg-[var(--surface)]"
                    title={t("settingsDiagramPrimaryColor")}
                  />
                  <span className="text-xs text-[var(--text-muted)]">{t("settingsDiagramPrimaryColor")}</span>
                </div>
              </div>
            </div>
          )}
        </div>
      )}
    </section>
  );
}
