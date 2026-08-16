import type { CatalogEntry, DiagramNode } from "../api/types";
import { applyVariantToNode, variantOptionsFor } from "../catalog/variants";
import { t } from "../i18n";

/** Multi-select Configurable (F63) + bulk param replace. */
export function SelectionToolsPanel({
  nodes,
  selectedIds,
  catalog,
  onApplyNodes,
}: {
  nodes: DiagramNode[];
  selectedIds: string[];
  catalog: CatalogEntry[];
  onApplyNodes: (next: DiagramNode[]) => void;
}) {
  const selected = nodes.filter((n) => selectedIds.includes(n.id) && n.nodeKind !== "draw");
  if (selected.length < 2) return null;

  const first = selected[0]!;
  const variants = variantOptionsFor(first.typeName, first.catalogId);
  const sameFamily = selected.every(
    (n) => variantOptionsFor(n.typeName, n.catalogId) === variants && variants != null,
  );

  const commonParamNames = (() => {
    if (!selected.length) return [] as string[];
    let names = new Set(selected[0]!.params.map((p) => p.name));
    for (const n of selected.slice(1)) {
      const s = new Set(n.params.map((p) => p.name));
      names = new Set([...names].filter((x) => s.has(x)));
    }
    return [...names].sort();
  })();

  return (
    <div className="selection-tools">
      <div className="selection-tools-title">
        {t("selectionTools")} ({selected.length})
      </div>
      {sameFamily && variants ? (
        <label className="selection-tools-row">
          <span>{t("configurableType")}</span>
          <select
            defaultValue=""
            onChange={(ev) => {
              const catalogId = ev.target.value;
              if (!catalogId) return;
              const nextVar = variants.find((v) => v.catalogId === catalogId);
              if (!nextVar) return;
              const entry = catalog.find((c) => c.id === nextVar.catalogId);
              const idSet = new Set(selectedIds);
              onApplyNodes(
                nodes.map((n) =>
                  idSet.has(n.id) ? applyVariantToNode(n, nextVar, entry) : n,
                ),
              );
              ev.target.value = "";
            }}
          >
            <option value="">{t("configurablePick")}</option>
            {variants.map((v) => (
              <option key={v.catalogId} value={v.catalogId}>
                {v.label}
              </option>
            ))}
          </select>
        </label>
      ) : (
        <div className="selection-tools-hint">{t("configurableMixed")}</div>
      )}
      {commonParamNames.length ? (
        <label className="selection-tools-row">
          <span>{t("bulkParam")}</span>
          <select
            id="bulk-param-name"
            defaultValue={commonParamNames[0]}
            onChange={() => {}}
          >
            {commonParamNames.map((name) => (
              <option key={name} value={name}>
                {name}
              </option>
            ))}
          </select>
          <input
            id="bulk-param-value"
            placeholder={t("bulkParamValue")}
            onKeyDown={(ev) => {
              if (ev.key !== "Enter") return;
              const nameEl = document.getElementById("bulk-param-name") as HTMLSelectElement | null;
              const valEl = ev.currentTarget;
              const pname = nameEl?.value;
              const value = valEl.value;
              if (!pname) return;
              const idSet = new Set(selectedIds);
              onApplyNodes(
                nodes.map((n) => {
                  if (!idSet.has(n.id)) return n;
                  const has = n.params.some((p) => p.name === pname);
                  const params = has
                    ? n.params.map((p) => (p.name === pname ? { ...p, value } : p))
                    : [...n.params, { name: pname, value }];
                  return { ...n, params };
                }),
              );
            }}
          />
        </label>
      ) : null}
      <div className="selection-tools-hint">{t("bulkParamHint")}</div>
    </div>
  );
}
