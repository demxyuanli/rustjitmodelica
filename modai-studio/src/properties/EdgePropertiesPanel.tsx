import { t } from "../i18n";
import type { DiagramDocument, DiagramEdge } from "../api/types";
import { edgeDomainOf } from "../canvas/edgeStyle";

/** Properties for a selected connection (signal attrs F72). */
export function EdgePropertiesPanel({
  edge,
  diagram,
  onChange,
}: {
  edge: DiagramEdge;
  diagram: DiagramDocument;
  onChange: (e: DiagramEdge) => void;
}) {
  const domain = edgeDomainOf(diagram, edge);

  return (
    <div className="panel right">
      <h3>{t("edgeProperties")}</h3>
      <div className="prop-row">
        <span>{t("edgeDomain")}</span>
        <span className={`edge-domain-chip domain-${domain}`}>{domain}</span>
      </div>
      <div className="prop-row">
        <span>{t("signalName")}</span>
        <input
          value={edge.signalName ?? ""}
          placeholder="gain.y"
          onChange={(ev) => onChange({ ...edge, signalName: ev.target.value || null })}
        />
      </div>
      <div className="prop-row">
        <span>{t("edgeUnit")}</span>
        <input
          value={edge.unit ?? ""}
          placeholder="V, A, N.m, 1"
          onChange={(ev) => onChange({ ...edge, unit: ev.target.value || null })}
        />
      </div>
      <div className="prop-row">
        <span>{t("edgeDimension")}</span>
        <input
          value={edge.dimension ?? ""}
          placeholder="1, 3, [2]"
          onChange={(ev) => onChange({ ...edge, dimension: ev.target.value || null })}
        />
      </div>
      <div className="hint" style={{ padding: "6px 10px" }}>
        {t("edgePropsHint")}
      </div>
      <div className="prop-row">
        <span>{t("edgeFrom")}</span>
        <span className="hint" style={{ wordBreak: "break-all" }}>
          {edge.sourceNodeId}.{edge.sourcePortId}
        </span>
      </div>
      <div className="prop-row">
        <span>{t("edgeTo")}</span>
        <span className="hint" style={{ wordBreak: "break-all" }}>
          {edge.targetNodeId}.{edge.targetPortId}
        </span>
      </div>
    </div>
  );
}
