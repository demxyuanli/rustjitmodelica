import { useMemo, useState } from "react";
import { t, tf } from "../i18n";
import { features, JIT_FEATURE_CATEGORIES, type JitFeature } from "../data/jit_regression_metadata";

function filterFeatures(featList: JitFeature[], search: string, category: string): JitFeature[] {
  const q = search.trim().toLowerCase();
  return featList.filter((f) => {
    if (category && f.category !== category) return false;
    if (!q) return true;
    return (
      f.id.toLowerCase().includes(q) ||
      f.name.toLowerCase().includes(q) ||
      f.description.toLowerCase().includes(q) ||
      f.category.toLowerCase().includes(q)
    );
  });
}

export function JitOverview() {
  const [search, setSearch] = useState("");
  const [categoryFilter, setCategoryFilter] = useState("");

  const filtered = useMemo(() => filterFeatures(features, search, categoryFilter), [search, categoryFilter]);
  const byCategory = useMemo(() => {
    const map = new Map<string, JitFeature[]>();
    for (const f of filtered) {
      const list = map.get(f.category) ?? [];
      list.push(f);
      map.set(f.category, list);
    }
    return map;
  }, [filtered]);

  const total = features.length;
  const covered = useMemo(() => features.filter((f) => f.status === "covered").length, []);
  const partial = useMemo(() => features.filter((f) => f.status === "partial").length, []);

  return (
    <div className="flex flex-col h-full min-h-0 overflow-auto p-4">
      <h2 className="text-base font-semibold text-[var(--text)] mb-2">{t("jitOverviewTitle")}</h2>
      <p className="text-xs text-[var(--text-muted)] mb-4">{t("jitOverviewDesc")}</p>

      <div className="flex flex-wrap gap-3 mb-4">
        <div className="rounded-lg border border-border bg-[var(--surface-elevated)] px-4 py-2 min-w-[80px]">
          <div className="text-[10px] uppercase text-[var(--text-muted)]">{t("jitSummaryTotal")}</div>
          <div className="text-lg font-semibold text-[var(--text)]">{total}</div>
        </div>
        <div className="rounded-lg border theme-banner-success px-4 py-2 min-w-[80px]">
          <div className="text-[10px] uppercase opacity-80">{t("jitSummaryCovered")}</div>
          <div className="text-lg font-semibold">{covered}</div>
        </div>
        <div className="rounded-lg border theme-banner-warning px-4 py-2 min-w-[80px]">
          <div className="text-[10px] uppercase opacity-80">{t("jitSummaryPartial")}</div>
          <div className="text-lg font-semibold">{partial}</div>
        </div>
      </div>

      <div className="flex gap-2 mb-4 flex-wrap">
        <input
          type="text"
          placeholder={t("search")}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="flex-1 min-w-[120px] max-w-[240px] theme-input border px-3 py-1.5 text-sm rounded-lg placeholder:text-[var(--text-muted)]"
        />
        <select
          value={categoryFilter}
          onChange={(e) => setCategoryFilter(e.target.value)}
          className="theme-input border px-3 py-1.5 text-sm rounded-lg text-[var(--text)]"
        >
          <option value="">{t("allCategories")}</option>
          {JIT_FEATURE_CATEGORIES.map((c) => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </select>
      </div>

      <div className="space-y-4">
        {JIT_FEATURE_CATEGORIES.map((cat) => {
          const list = byCategory.get(cat);
          if (!list?.length) return null;
          return (
            <section key={cat} className="rounded-lg border border-border bg-[var(--surface-elevated)] overflow-hidden">
              <h3 className="text-sm font-medium px-4 py-2.5 bg-[var(--panel-muted-bg)] text-[var(--text)] border-b border-border rounded-t-lg">
                {cat}
              </h3>
              <div className="overflow-x-auto">
                <table className="w-full text-xs">
                  <thead>
                    <tr className="text-left text-[var(--text-muted)] border-b border-border">
                      <th className="px-4 py-2 font-medium w-20">{t("id")}</th>
                      <th className="px-4 py-2 font-medium">{t("name")}</th>
                      <th className="px-4 py-2 font-medium flex-1">{t("description")}</th>
                      <th className="px-4 py-2 font-medium w-24">{t("status")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {list.map((f) => (
                      <tr key={f.id} className="border-b border-border/50 hover:bg-[var(--surface-hover)]">
                        <td className="px-4 py-2 font-mono text-[var(--text)]">{f.id}</td>
                        <td className="px-4 py-2 text-[var(--text)]">{f.name}</td>
                        <td className="px-4 py-2 text-[var(--text-muted)]">{f.description}</td>
                        <td className="px-4 py-2">
                          <span
                            className={
                              f.status === "covered"
                                ? "px-2 py-0.5 rounded-lg theme-banner-success text-[10px]"
                                : "px-2 py-0.5 rounded-lg theme-banner-warning text-[10px]"
                            }
                          >
                            {f.status === "covered" ? t("statusCovered") : t("statusPartial")}
                          </span>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </section>
          );
        })}
      </div>
      <div className="mt-4 text-xs text-[var(--text-muted)]">
        {t("jitFeatureCount")}: {filtered.length}
        {filtered.length !== total && ` (${tf("filteredFrom", { total })})`}
      </div>
    </div>
  );
}
