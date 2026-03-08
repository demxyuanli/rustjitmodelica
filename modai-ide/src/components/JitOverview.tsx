import { useMemo, useState } from "react";
import { t } from "../i18n";
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
        <div className="rounded-lg border border-gray-700 bg-[#2d2d2d] px-4 py-2 min-w-[80px]">
          <div className="text-[10px] uppercase text-[var(--text-muted)]">{t("jitSummaryTotal")}</div>
          <div className="text-lg font-semibold text-[var(--text)]">{total}</div>
        </div>
        <div className="rounded-lg border border-green-700/50 bg-green-900/20 px-4 py-2 min-w-[80px]">
          <div className="text-[10px] uppercase text-green-400/80">{t("jitSummaryCovered")}</div>
          <div className="text-lg font-semibold text-green-300">{covered}</div>
        </div>
        <div className="rounded-lg border border-amber-700/50 bg-amber-900/20 px-4 py-2 min-w-[80px]">
          <div className="text-[10px] uppercase text-amber-400/80">{t("jitSummaryPartial")}</div>
          <div className="text-lg font-semibold text-amber-300">{partial}</div>
        </div>
      </div>

      <div className="flex gap-2 mb-4 flex-wrap">
        <input
          type="text"
          placeholder={t("search")}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="flex-1 min-w-[120px] max-w-[240px] bg-[#3c3c3c] border border-gray-600 px-3 py-1.5 text-sm rounded-lg placeholder:text-[var(--text-muted)]"
        />
        <select
          value={categoryFilter}
          onChange={(e) => setCategoryFilter(e.target.value)}
          className="bg-[#3c3c3c] border border-gray-600 px-3 py-1.5 text-sm rounded-lg text-[var(--text)]"
        >
          <option value="">All categories</option>
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
            <section key={cat} className="rounded-lg border border-gray-700 bg-[#2d2d2d] overflow-hidden">
              <h3 className="text-sm font-medium px-4 py-2.5 bg-[#3c3c3c] text-[var(--text)] border-b border-gray-700 rounded-t-lg">
                {cat}
              </h3>
              <div className="overflow-x-auto">
                <table className="w-full text-xs">
                  <thead>
                    <tr className="text-left text-[var(--text-muted)] border-b border-gray-700">
                      <th className="px-4 py-2 font-medium w-20">ID</th>
                      <th className="px-4 py-2 font-medium">Name</th>
                      <th className="px-4 py-2 font-medium flex-1">Description</th>
                      <th className="px-4 py-2 font-medium w-24">Status</th>
                    </tr>
                  </thead>
                  <tbody>
                    {list.map((f) => (
                      <tr key={f.id} className="border-b border-gray-700/50 hover:bg-[#3c3c3c]/30">
                        <td className="px-4 py-2 font-mono text-[var(--text)]">{f.id}</td>
                        <td className="px-4 py-2 text-[var(--text)]">{f.name}</td>
                        <td className="px-4 py-2 text-[var(--text-muted)]">{f.description}</td>
                        <td className="px-4 py-2">
                          <span
                            className={
                              f.status === "covered"
                                ? "px-2 py-0.5 rounded-lg bg-green-900/50 text-green-300 text-[10px]"
                                : "px-2 py-0.5 rounded-lg bg-amber-900/50 text-amber-300 text-[10px]"
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
        {filtered.length !== total && ` (filtered from ${total})`}
      </div>
    </div>
  );
}
