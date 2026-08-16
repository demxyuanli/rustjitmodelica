import { useEffect, useMemo, useState } from "react";
import { Star } from "lucide-react";
import type { CatalogEntry, DiagramNode } from "../api/types";
import { BlockIcon, blockIconKey } from "../canvas/blockIcons";
import { t } from "../i18n";
import { catalogCategoryLabel, catalogDomainHint, catalogLabel } from "./catalogLabel";
import { setCatalogDragData } from "./catalogDnd";
import { loadFavorites, toggleFavorite } from "./favorites";
import { loadRecents, rememberRecent } from "./recents";

const DEFAULT_OPEN = new Set(["Sources", "Math", "Continuous", "Analog", "StateGraph"]);

function entryAsNode(e: CatalogEntry): DiagramNode {
  return {
    id: e.id,
    instanceName: e.defaultInstancePrefix,
    typeName: e.typeName,
    domain: e.domain,
    catalogId: e.id,
    position: { x: 0, y: 0 },
    rotation: 0,
    params: [],
    ports: e.ports,
    nodeKind: "block",
    subsystemRef: null,
    plotHints: e.plotHints ?? [],
  };
}

export function LibraryPanel({
  catalog,
  filter,
  onFilter,
  selectedId,
  onSelect,
  recentTick,
}: {
  catalog: CatalogEntry[];
  filter: string;
  onFilter: (v: string) => void;
  selectedId: string | null;
  onSelect: (e: CatalogEntry) => void;
  recentTick?: number;
}) {
  const [favorites, setFavorites] = useState<string[]>(() => loadFavorites());
  const [recents, setRecents] = useState<string[]>(() => loadRecents());
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const favSet = useMemo(() => new Set(favorites), [favorites]);

  useEffect(() => {
    setRecents(loadRecents());
  }, [recentTick]);

  const q = filter.trim().toLowerCase();
  const filtered = catalog.filter(
    (c) =>
      !q ||
      c.id.toLowerCase().includes(q) ||
      c.typeName.toLowerCase().includes(q) ||
      c.category.toLowerCase().includes(q) ||
      c.defaultInstancePrefix.toLowerCase().includes(q) ||
      catalogLabel(c).toLowerCase().includes(q),
  );

  const byCat = new Map<string, CatalogEntry[]>();
  for (const e of filtered) {
    const list = byCat.get(e.category) ?? [];
    list.push(e);
    byCat.set(e.category, list);
  }

  const favEntries = filtered.filter((e) => favSet.has(e.id));
  const recentEntries = recents
    .map((id) => filtered.find((e) => e.id === id))
    .filter((e): e is CatalogEntry => !!e);

  const pick = (e: CatalogEntry) => {
    setRecents((prev) => rememberRecent(e.id, prev));
    onSelect(e);
  };

  const isOpen = (cat: string) => {
    if (q) return true;
    if (collapsed[cat] != null) return !collapsed[cat];
    return DEFAULT_OPEN.has(cat);
  };

  const renderItem = (e: CatalogEntry) => (
    <div key={e.id} className="catalog-item-row">
      <button
        type="button"
        className="catalog-item"
        draggable
        style={{
          borderColor: selectedId === e.id ? "var(--accent)" : "transparent",
        }}
        onClick={() => pick(e)}
        onDragStart={(ev) => {
          setCatalogDragData(ev.dataTransfer, e.id);
          setRecents((prev) => rememberRecent(e.id, prev));
        }}
      >
        <span className="catalog-icon">
          <BlockIcon kind={blockIconKey(entryAsNode(e))} />
        </span>
        <span className="catalog-item-text">
          <span className="catalog-item-name">{catalogLabel(e)}</span>
          <span className="catalog-item-meta">{catalogDomainHint(e)}</span>
        </span>
      </button>
      <button
        type="button"
        className={`catalog-fav${favSet.has(e.id) ? " on" : ""}`}
        title={t("toggleFavorite")}
        onClick={(ev) => {
          ev.stopPropagation();
          setFavorites((prev) => toggleFavorite(e.id, prev));
        }}
      >
        {favSet.has(e.id) ? <Star size={14} fill="currentColor" strokeWidth={1.5} /> : <Star size={14} strokeWidth={1.5} />}
      </button>
    </div>
  );

  const renderCat = (cat: string, items: CatalogEntry[]) => {
    const open = isOpen(cat);
    return (
      <div key={cat}>
        <button
          type="button"
          className="catalog-cat"
          onClick={() => setCollapsed((prev) => ({ ...prev, [cat]: open }))}
        >
          <span className="catalog-cat-chevron">{open ? "\u25BE" : "\u25B8"}</span>
          {catalogCategoryLabel(cat)}
          <span className="catalog-cat-count">{items.length}</span>
        </button>
        {open ? items.map(renderItem) : null}
      </div>
    );
  };

  return (
    <div className="panel">
      <h3>{t("library")}</h3>
      <div style={{ padding: "8px 10px" }}>
        <input
          style={{
            width: "100%",
            background: "var(--surface-input)",
            color: "var(--text)",
            border: "1px solid var(--border)",
            borderRadius: 4,
            padding: "4px 6px",
          }}
          placeholder={t("searchPlaceholder")}
          value={filter}
          onChange={(ev) => onFilter(ev.target.value)}
        />
        {selectedId ? (
          <div className="hint" style={{ marginTop: 6, color: "var(--accent)" }}>
            {t("libraryPlaceHint")}
          </div>
        ) : (
          <div className="hint" style={{ marginTop: 6 }}>
            {t("libraryIdleHint")}
          </div>
        )}
      </div>
      {recentEntries.length > 0 ? renderCat("Recents", recentEntries) : null}
      {favEntries.length > 0 ? renderCat("Favorites", favEntries) : null}
      {[...byCat.entries()].map(([cat, items]) => renderCat(cat, items))}
    </div>
  );
}
