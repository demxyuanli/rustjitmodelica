import { useMemo, useState } from "react";
import type { DiagramNode } from "../api/types";
import { t } from "../i18n";
import { groupSeries, type SeriesDisplay } from "./seriesDisplay";

export function VarTree({
  keys,
  selectedVars,
  pendingVars,
  nodes,
  onToggle,
}: {
  keys: string[];
  selectedVars: string[];
  pendingVars: string[];
  nodes: DiagramNode[];
  onToggle: (key: string) => void;
}) {
  const [q, setQ] = useState("");
  const groups = useMemo(() => groupSeries(keys.length ? keys : pendingVars, nodes), [keys, pendingVars, nodes]);
  const needle = q.trim().toLowerCase();

  const visible = groups
    .map((g) => ({
      ...g,
      items: g.items.filter((it) => {
        if (!needle) return true;
        return (
          it.label.toLowerCase().includes(needle) ||
          it.key.toLowerCase().includes(needle) ||
          g.instance.toLowerCase().includes(needle)
        );
      }),
    }))
    .filter((g) => g.items.length);

  const checked = (it: SeriesDisplay) =>
    selectedVars.includes(it.key) || selectedVars.includes(it.label);

  if (!keys.length && !pendingVars.length) {
    return <span style={{ color: "var(--muted)" }}>{t("resultsNoSeries")}</span>;
  }

  return (
    <div className="var-tree">
      <input
        className="var-tree-search"
        placeholder={t("varSearch")}
        value={q}
        onChange={(ev) => setQ(ev.target.value)}
      />
      {!keys.length && pendingVars.length ? (
        <div className="var-tree-hint">{t("resultsAwaitingRun")}</div>
      ) : null}
      {visible.map((g) => (
        <div key={g.instance || "_other"} className="var-tree-group">
          <div className="var-tree-head">{g.instance || t("varOther")}</div>
          {g.items.map((it) => (
            <label key={it.key} className="var-tree-item" title={it.key}>
              <input type="checkbox" checked={checked(it)} onChange={() => onToggle(it.key)} />
              <span>{it.label}</span>
            </label>
          ))}
        </div>
      ))}
    </div>
  );
}
