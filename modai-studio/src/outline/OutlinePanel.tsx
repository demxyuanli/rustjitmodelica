import type { DiagramNode } from "../api/types";

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
      <h3>Outline</h3>
      {sorted.length === 0 ? (
        <div style={{ padding: 10, fontSize: 12, color: "var(--muted)" }}>No instances</div>
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
            <div style={{ fontSize: 10, color: "var(--muted)" }}>
              {n.typeName.split(".").slice(-1)[0]}
            </div>
          </button>
        ))
      )}
    </div>
  );
}
