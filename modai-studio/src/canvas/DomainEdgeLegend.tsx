import { t } from "../i18n";

/** Compact F72 domain connection style legend. */
export function DomainEdgeLegend() {
  return (
    <div className="domain-edge-legend" title={t("domainEdgeLegend")}>
      <div className="domain-edge-legend-title">{t("domainEdgeLegend")}</div>
      <div className="domain-edge-legend-row">
        <span className="swatch signal" />
        <span>{t("domainSignal")}</span>
      </div>
      <div className="domain-edge-legend-row">
        <span className="swatch electrical" />
        <span>{t("domainElectrical")}</span>
      </div>
      <div className="domain-edge-legend-row">
        <span className="swatch mechanical" />
        <span>{t("domainMechanical")}</span>
      </div>
      <div className="domain-edge-legend-row">
        <span className="swatch multibody" />
        <span>{t("domainMultibody")}</span>
      </div>
    </div>
  );
}
