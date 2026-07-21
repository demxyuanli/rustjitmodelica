import type { CatalogEntry } from "../api/types";

export function LibraryPanel({
  catalog,
  filter,
  onFilter,
  selectedId,
  onSelect,
}: {
  catalog: CatalogEntry[];
  filter: string;
  onFilter: (v: string) => void;
  selectedId: string | null;
  onSelect: (e: CatalogEntry) => void;
}) {
  const q = filter.trim().toLowerCase();
  const filtered = catalog.filter(
    (c) =>
      !q ||
      c.id.toLowerCase().includes(q) ||
      c.typeName.toLowerCase().includes(q) ||
      c.category.toLowerCase().includes(q),
  );
  const byCat = new Map<string, CatalogEntry[]>();
  for (const e of filtered) {
    const list = byCat.get(e.category) ?? [];
    list.push(e);
    byCat.set(e.category, list);
  }

  return (
    <div className="panel">
      <h3>Library</h3>
      <div style={{ padding: "8px 10px" }}>
        <input
          style={{ width: "100%", background: "#15191e", color: "var(--text)", border: "1px solid var(--border)", borderRadius: 4, padding: "4px 6px" }}
          placeholder="Search..."
          value={filter}
          onChange={(ev) => onFilter(ev.target.value)}
        />
        {selectedId ? (
          <div style={{ marginTop: 6, fontSize: 11, color: "var(--accent)" }}>
            Click canvas to place: {selectedId}
          </div>
        ) : (
          <div style={{ marginTop: 6, fontSize: 11, color: "var(--muted)" }}>
            Select a block, then click canvas
          </div>
        )}
      </div>
      {[...byCat.entries()].map(([cat, items]) => (
        <div key={cat}>
          <div className="catalog-cat">{cat}</div>
          {items.map((e) => (
            <button
              key={e.id}
              type="button"
              className="catalog-item"
              style={{
                borderColor: selectedId === e.id ? "var(--accent)" : "transparent",
              }}
              onClick={() => onSelect(e)}
            >
              {e.defaultInstancePrefix}
              <div style={{ fontSize: 10, color: "var(--muted)" }}>{e.domain}</div>
            </button>
          ))}
        </div>
      ))}
    </div>
  );
}
