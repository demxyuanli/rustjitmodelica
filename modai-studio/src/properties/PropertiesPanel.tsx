import type { CatalogEntry, DiagramNode } from "../api/types";
import { variantOptionsFor } from "../catalog/variants";

export function PropertiesPanel({
  node,
  catalog,
  onChange,
}: {
  node: DiagramNode | null;
  catalog: CatalogEntry[];
  onChange: (n: DiagramNode) => void;
}) {
  if (!node) {
    return (
      <div className="panel right">
        <h3>Properties</h3>
        <div style={{ padding: 10, color: "var(--muted)", fontSize: 13 }}>Select a block</div>
      </div>
    );
  }

  const variants = variantOptionsFor(node.typeName);

  return (
    <div className="panel right">
      <h3>Properties</h3>
      <div className="prop-row">
        <span>Name</span>
        <input
          value={node.instanceName}
          onChange={(ev) =>
            onChange({
              ...node,
              instanceName: ev.target.value,
              ui: { ...node.ui, label: ev.target.value },
            })
          }
        />
      </div>
      <div className="prop-row">
        <span>Type</span>
        {variants ? (
          <select
            value={node.typeName}
            onChange={(ev) => {
              const next = variants.find((v) => v.typeName === ev.target.value);
              if (!next) return;
              const entry = catalog.find((c) => c.id === next.catalogId);
              onChange({
                ...node,
                typeName: next.typeName,
                catalogId: next.catalogId,
                params: entry
                  ? entry.defaultParams.map((p) => {
                      const prev = node.params.find((x) => x.name === p.name);
                      return { name: p.name, value: prev?.value ?? p.value };
                    })
                  : node.params,
                ports: entry ? entry.ports.map((p) => ({ ...p })) : node.ports,
                plotHints: entry ? [...entry.plotHints] : node.plotHints,
                domain: entry?.domain ?? node.domain,
              });
            }}
          >
            {variants.map((v) => (
              <option key={v.typeName} value={v.typeName}>
                {v.label}
              </option>
            ))}
          </select>
        ) : (
          <span style={{ fontSize: 11, color: "var(--muted)", wordBreak: "break-all" }}>
            {node.typeName}
          </span>
        )}
      </div>
      {node.params.map((p, idx) => (
        <div className="prop-row" key={p.name}>
          <span>{p.name}</span>
          <input
            value={p.value}
            onChange={(ev) => {
              const params = node.params.map((x, i) =>
                i === idx ? { ...x, value: ev.target.value } : x,
              );
              onChange({ ...node, params });
            }}
          />
        </div>
      ))}
    </div>
  );
}
