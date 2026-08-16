import { t } from "../i18n";

/** Compact F73 sample-time color legend. */
export function SampleTimeLegend() {
  return (
    <div className="sample-time-legend" title={t("sampleTimeLegend")}>
      <div className="sample-time-legend-title">{t("sampleTimeLegend")}</div>
      <div className="sample-time-legend-row">
        <span className="swatch continuous" />
        <span>{t("sampleContinuous")}</span>
      </div>
      <div className="sample-time-legend-row">
        <span className="swatch discrete" />
        <span>{t("sampleDiscrete")}</span>
      </div>
      <div className="sample-time-legend-row">
        <span className="swatch inherit" />
        <span>{t("sampleInherit")}</span>
      </div>
    </div>
  );
}
