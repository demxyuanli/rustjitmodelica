import type { DiagramNode } from "../api/types";
import { t } from "../i18n";

export function OutlinePanel({
  nodes,
  selectedIds,
  onSelect,
}: {
  nodes: DiagramNode[];
  selectedIds: string[];
  onSelect: (id: string) => void;
}) {
  const sorted = [...nodes].sort((a, b) => a.instanceName.localeCompare(b.instanceName));
  return (
    <div>
      <h3>{t("outline")}</h3>
      {sorted.length === 0 ? (
        <div className="hint" style={{ padding: 10 }}>{t("noInstances")}</div>
      ) : (
        sorted.map((n) => (
          <button
            key={n.id}
            type="button"
            className="catalog-item"
            style={{
              borderColor: selectedIds.includes(n.id) ? "var(--accent)" : "transparent",
            }}
            onClick={() => onSelect(n.id)}
          >
            {n.instanceName}
            <div className="hint">
              {n.typeName.split(".").slice(-1)[0]}
            </div>
          </button>
        ))
      )}
    </div>
  );
}
