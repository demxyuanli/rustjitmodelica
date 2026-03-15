import type { DiagramColorScheme } from "../utils/diagramColorSchemes";
import { DEFAULT_CONNECTOR_KEYS } from "../utils/diagramColorSchemes";
import { t } from "../i18n";

export interface DiagramColorSchemePreviewProps {
  scheme: DiagramColorScheme;
  compact?: boolean;
}

const BG_ELEVATED = "var(--bg-elevated)";
const BORDER_MUTED = "var(--text-muted)";

function MiniDiagram({ scheme }: { scheme: DiagramColorScheme }) {
  const primary = scheme.diagramPrimary ?? "#3b82f6";
  const border = BORDER_MUTED;
  return (
    <svg
      width="100%"
      height="44"
      className="block shrink-0"
      viewBox="0 0 130 44"
      preserveAspectRatio="xMidYMid meet"
    >
      <rect x="6" y="10" width="40" height="24" rx="2" fill={BG_ELEVATED} stroke={primary} strokeWidth="1.5" />
      <line x1="50" y1="22" x2="80" y2="22" stroke={border} strokeWidth="1.2" />
      <polygon points="80,18 90,22 80,26" fill={border} />
      <rect x="94" y="10" width="30" height="24" rx="2" fill={BG_ELEVATED} stroke={border} strokeWidth="1" />
    </svg>
  );
}

export function DiagramColorSchemePreview({ scheme, compact = false }: DiagramColorSchemePreviewProps) {
  const connectorKeys = DEFAULT_CONNECTOR_KEYS;
  const chartColors = (scheme.chartPaletteDark ?? scheme.chartPaletteLight).slice(0, 8);

  if (compact) {
    return (
      <div
        className="rounded border border-[var(--border)] bg-[var(--surface)] p-2 flex flex-col items-stretch"
        style={{ minWidth: 112, width: 112 }}
      >
        <MiniDiagram scheme={scheme} />
        <div className="mt-1.5 flex flex-wrap gap-0.5 justify-center">
          {connectorKeys.slice(0, 4).map((key) => (
            <span
              key={key}
              className="inline-block h-2.5 w-2.5 rounded-sm border border-[var(--border)]"
              style={{ backgroundColor: scheme.connectorColors[key] ?? "#888" }}
              title={key}
            />
          ))}
        </div>
        <div className="mt-1 flex gap-0.5">
          {chartColors.slice(0, 6).map((color, i) => (
            <span key={i} className="h-2 flex-1 min-w-0 rounded-sm" style={{ backgroundColor: color }} />
          ))}
        </div>
        <div className="mt-1 text-[10px] text-[var(--text-muted)] text-center truncate">
          {t(scheme.labelKey)}
        </div>
      </div>
    );
  }

  return (
    <div className="rounded border border-[var(--border)] bg-[var(--surface)] p-2" style={{ minWidth: 200 }}>
      <MiniDiagram scheme={scheme} />
      <div className="mt-2 flex flex-wrap gap-1">
        {connectorKeys.map((key) => (
          <span
            key={key}
            className="inline-block h-4 w-4 rounded border border-[var(--border)]"
            style={{ backgroundColor: scheme.connectorColors[key] ?? "#888" }}
            title={key}
          />
        ))}
      </div>
      <div className="mt-1.5 flex gap-0.5">
        {chartColors.map((color, i) => (
          <span key={i} className="h-3 flex-1 min-w-[6px] rounded-sm" style={{ backgroundColor: color }} />
        ))}
      </div>
      <div className="mt-1.5 text-[10px] text-[var(--text-muted)]">{t(scheme.labelKey)}</div>
    </div>
  );
}
