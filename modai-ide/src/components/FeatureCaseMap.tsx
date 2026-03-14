import { useMemo, useState } from "react";
import { t } from "../i18n";
import { IconButton } from "./IconButton";
import { AppIcon } from "./Icon";
import {
  features,
  cases,
  featureToCases,
  caseToFeatures,
  isCaseCoveringFeature,
} from "../data/jit_regression_metadata";

const CASE_PAGE_SIZE = 12;
const FEATURE_PAGE_SIZE = 15;

const GRAPH_LEFT_X = 140;
const GRAPH_RIGHT_X = 560;
const GRAPH_ROW = 20;
const GRAPH_RADIUS = 6;

export function FeatureCaseMap() {
  const [viewMode, setViewMode] = useState<"table" | "graph">("table");
  const [casePage, setCasePage] = useState(0);
  const [featurePage, setFeaturePage] = useState(0);
  const [highlightFeature, setHighlightFeature] = useState<string | null>(null);
  const [highlightCase, setHighlightCase] = useState<string | null>(null);

  const caseSlice = useMemo(() => {
    const start = casePage * CASE_PAGE_SIZE;
    return cases.slice(start, start + CASE_PAGE_SIZE);
  }, [casePage]);

  const featureSlice = useMemo(() => {
    const start = featurePage * FEATURE_PAGE_SIZE;
    return features.slice(start, start + FEATURE_PAGE_SIZE);
  }, [featurePage]);

  const maxCasePage = Math.max(0, Math.ceil(cases.length / CASE_PAGE_SIZE) - 1);
  const maxFeaturePage = Math.max(0, Math.ceil(features.length / FEATURE_PAGE_SIZE) - 1);

  return (
    <div className="flex flex-col h-full min-h-0 overflow-auto p-4">
      <h2 className="text-base font-semibold text-[var(--text)] mb-2">{t("featureCaseMapTitle")}</h2>
      <p className="text-xs text-[var(--text-muted)] mb-4">{t("featureCaseMapDesc")}</p>

      <div className="flex items-center gap-4 mb-4 flex-wrap">
        <div className="flex rounded-lg border border-border overflow-hidden" role="group" aria-label={t("viewMode")}>
          <IconButton
            icon={<AppIcon name="table" aria-hidden="true" />}
            variant="tab"
            size="xs"
            active={viewMode === "table"}
            onClick={() => setViewMode("table")}
            title={t("viewModeTable")}
            aria-label={t("viewModeTable")}
          />
          <IconButton
            icon={<AppIcon name="chart" aria-hidden="true" />}
            variant="tab"
            size="xs"
            active={viewMode === "graph"}
            onClick={() => setViewMode("graph")}
            title={t("viewModeGraph")}
            aria-label={t("viewModeGraph")}
          />
        </div>
        <div className="flex gap-4 flex-wrap">
        <div className="flex items-center gap-2">
          <span className="text-xs text-[var(--text-muted)]">{t("viewByFeature")}:</span>
          <select
            className="text-xs theme-input border rounded px-2 py-1 text-[var(--text)]"
            value={highlightFeature ?? ""}
            onChange={(e) => {
              setHighlightFeature(e.target.value || null);
              setHighlightCase(null);
            }}
          >
            <option value="">--</option>
            {features.map((f) => (
              <option key={f.id} value={f.id}>
                {f.id}: {f.name}
              </option>
            ))}
          </select>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-xs text-[var(--text-muted)]">{t("viewByCase")}:</span>
          <select
            className="text-xs theme-input border rounded px-2 py-1 text-[var(--text)] max-w-[220px]"
            value={highlightCase ?? ""}
            onChange={(e) => {
              setHighlightCase(e.target.value || null);
              setHighlightFeature(null);
            }}
          >
            <option value="">--</option>
            {cases.map((c) => (
              <option key={c.name} value={c.name}>
                {c.name}
              </option>
            ))}
            </select>
        </div>
        </div>
      </div>

      {highlightFeature && (
        <div className="mb-4 p-3 rounded bg-[var(--surface-elevated)] border border-border text-xs">
          <span className="text-[var(--text-muted)]">{t("casesCoveringFeature")}: </span>
          <span className="text-[var(--text)]">
            {(featureToCases[highlightFeature] ?? []).join(", ") || t("none")}
          </span>
        </div>
      )}

      {highlightCase && (
        <div className="mb-4 p-3 rounded bg-[var(--surface-elevated)] border border-border text-xs">
          <span className="text-[var(--text-muted)]">{t("featuresCoveredByCase")}: </span>
          <span className="text-[var(--text)]">
            {(caseToFeatures[highlightCase] ?? []).join(", ") || t("none")}
          </span>
        </div>
      )}

      {viewMode === "graph" ? (
        <div className="rounded-lg border border-border bg-[var(--panel-muted-bg)] overflow-auto flex-1 min-h-0 flex flex-col">
          <div className="text-xs text-[var(--text-muted)] px-3 py-2 border-b border-border shrink-0 flex items-center gap-4 flex-wrap">
            <span>{t("featureCaseMapDesc")} ({features.length} features, {cases.length} cases)</span>
            <span className="flex items-center gap-3 ml-auto">
              <span className="flex items-center gap-1.5">
                <span className="w-3 h-3 rounded-full bg-green-500" aria-hidden />
                <span>{t("graphLegendFeature")}</span>
              </span>
              <span className="flex items-center gap-1.5">
                <span className="w-3 h-3 rounded-full bg-blue-500" aria-hidden />
                <span>{t("graphLegendCase")}</span>
              </span>
              <span className="flex items-center gap-1.5">
                <span className="w-6 h-0.5 bg-[var(--border-strong)] rounded" aria-hidden />
                <span>{t("graphLegendEdge")}</span>
              </span>
            </span>
          </div>
          <div className="flex-1 min-h-0 overflow-auto p-2">
            <FeatureCaseGraph
              features={features}
              cases={cases}
              featureToCases={featureToCases}
              highlightFeature={highlightFeature}
              highlightCase={highlightCase}
              onHighlightFeature={setHighlightFeature}
              onHighlightCase={setHighlightCase}
            />
          </div>
        </div>
      ) : (
      <div className="rounded-lg border border-border bg-[var(--panel-muted-bg)] overflow-hidden flex-1 min-h-0 flex flex-col">
        <div className="text-xs text-[var(--text-muted)] px-3 py-2 border-b border-border">
          {t("featureCaseMatrix")} ({featureSlice.length} x {caseSlice.length})
        </div>
        <div className="flex-1 min-h-0 overflow-auto">
          <table className="w-full text-[10px] border-collapse">
            <thead className="sticky top-0 bg-[var(--surface-elevated)] z-10">
              <tr>
                <th className="px-2 py-1.5 text-left font-medium text-[var(--text-muted)] border border-border w-24 sticky left-0 bg-[var(--surface-elevated)]">
                  {t("viewByFeature")}
                </th>
                {caseSlice.map((c) => (
                  <th
                    key={c.name}
                    className={`px-1 py-1.5 text-center font-medium border border-border max-w-[80px] truncate align-bottom ${
                      highlightCase === c.name ? "bg-primary/20 text-primary" : "text-[var(--text-muted)]"
                    }`}
                    title={c.name}
                  >
                    {c.name.replace("TestLib/", "")}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {featureSlice.map((f) => (
                <tr
                  key={f.id}
                  className={highlightFeature === f.id ? "bg-primary/10" : ""}
                >
                  <td
                    className={`px-2 py-1 border border-border sticky left-0 bg-[var(--surface-elevated)] ${
                      highlightFeature === f.id ? "bg-blue-900/30" : ""
                    }`}
                    title={f.name}
                  >
                    <span className="font-mono text-[var(--text)]">{f.id}</span>
                  </td>
                  {caseSlice.map((c) => {
                    const covered = isCaseCoveringFeature(c.name, f.id);
                    return (
                      <td
                        key={c.name}
                        className={`px-1 py-1 border border-border text-center ${
                          highlightCase === c.name ? "bg-primary/10" : ""
                        }`}
                      >
                        {covered ? (
                          <span className="text-[var(--success-text)]" title={`${f.id} <-> ${c.name}`}>
                            &#x2713;
                          </span>
                        ) : (
                          <span className="text-[var(--text-muted)]">-</span>
                        )}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="flex items-center justify-between px-3 py-2 border-t border-border text-xs text-[var(--text-muted)]">
          <div className="flex gap-2 items-center">
            <button
              type="button"
              className="px-2 py-1 rounded border theme-button-secondary disabled:opacity-50 disabled:pointer-events-none"
              onClick={() => setFeaturePage((p) => Math.max(0, p - 1))}
              disabled={featurePage === 0}
            >
              {t("prevFeatures")}
            </button>
            <span>
              {featurePage * FEATURE_PAGE_SIZE + 1}-{Math.min((featurePage + 1) * FEATURE_PAGE_SIZE, features.length)} / {features.length}
            </span>
            <button
              type="button"
              className="px-2 py-1 rounded border theme-button-secondary disabled:opacity-50 disabled:pointer-events-none"
              onClick={() => setFeaturePage((p) => Math.min(maxFeaturePage, p + 1))}
              disabled={featurePage >= maxFeaturePage}
            >
              {t("nextFeatures")}
            </button>
          </div>
          <div className="flex gap-2 items-center">
            <button
              type="button"
              className="px-2 py-1 rounded border theme-button-secondary disabled:opacity-50 disabled:pointer-events-none"
              onClick={() => setCasePage((p) => Math.max(0, p - 1))}
              disabled={casePage === 0}
            >
              {t("prevCases")}
            </button>
            <span>
              {casePage * CASE_PAGE_SIZE + 1}-{Math.min((casePage + 1) * CASE_PAGE_SIZE, cases.length)} / {cases.length}
            </span>
            <button
              type="button"
              className="px-2 py-1 rounded border theme-button-secondary disabled:opacity-50 disabled:pointer-events-none"
              onClick={() => setCasePage((p) => Math.min(maxCasePage, p + 1))}
              disabled={casePage >= maxCasePage}
            >
              {t("nextCases")}
            </button>
          </div>
        </div>
      </div>
      )}
    </div>
  );
}

function FeatureCaseGraph({
  features: featList,
  cases: caseList,
  featureToCases: f2c,
  highlightFeature,
  highlightCase,
  onHighlightFeature,
  onHighlightCase,
}: {
  features: typeof features;
  cases: typeof cases;
  featureToCases: Record<string, string[]>;
  highlightFeature: string | null;
  highlightCase: string | null;
  onHighlightFeature: (id: string | null) => void;
  onHighlightCase: (name: string | null) => void;
}) {
  const svgWidth = 720;
  const rowH = GRAPH_ROW;
  const svgHeight = 40 + Math.max(featList.length, caseList.length) * rowH;
  const leftX = GRAPH_LEFT_X;
  const rightX = GRAPH_RIGHT_X;
  const midX = (leftX + rightX) / 2;

  const edges: { fi: number; ci: number }[] = [];
  featList.forEach((f, fi) => {
    (f2c[f.id] ?? []).forEach((cName) => {
      const ci = caseList.findIndex((c) => c.name === cName);
      if (ci >= 0) edges.push({ fi, ci });
    });
  });

  const fy = (i: number) => 28 + i * rowH;
  const cy = (j: number) => 28 + j * rowH;

  return (
    <svg
      width={svgWidth}
      height={svgHeight}
      viewBox={`0 0 ${svgWidth} ${svgHeight}`}
      className="max-w-full"
      style={{ minWidth: svgWidth, minHeight: Math.min(svgHeight, 500) }}
    >
      <defs>
        <marker
          id="arrowhead"
          markerWidth="6"
          markerHeight="4"
          refX="5"
          refY="2"
          orient="auto"
        >
          <polygon points="0 0, 6 2, 0 4" fill="var(--text-muted)" />
        </marker>
        <linearGradient id="edgeGrad" x1="0" y1="0" x2="1" y2="0">
          <stop offset="0" stopColor="#4ade80" />
          <stop offset="1" stopColor="#60a5fa" />
        </linearGradient>
      </defs>
      {edges.map(({ fi, ci }, k) => {
        const y1 = fy(fi);
        const y2 = cy(ci);
        const midY = (y1 + y2) / 2;
        const hl = highlightFeature === featList[fi].id || highlightCase === caseList[ci].name;
        return (
          <path
            key={`${fi}-${ci}-${k}`}
            d={`M ${leftX} ${y1} Q ${midX} ${midY} ${rightX} ${y2}`}
            fill="none"
            stroke={hl ? "url(#edgeGrad)" : "rgba(100,100,100,0.4)"}
            strokeWidth={hl ? 2 : 1}
            strokeDasharray={hl ? "none" : "2,2"}
          />
        );
      })}
      {featList.map((f, i) => {
        const y = fy(i);
        const hl = highlightFeature === f.id;
        return (
          <g
            key={f.id}
            role="button"
            tabIndex={0}
            aria-label={`${t("graphLegendFeature")} ${f.id}: ${f.name}`}
            style={{ cursor: "pointer" }}
            onClick={() => {
              onHighlightCase(null);
              onHighlightFeature(hl ? null : f.id);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onHighlightCase(null);
                onHighlightFeature(hl ? null : f.id);
              }
            }}
          >
            <circle
              cx={leftX}
              cy={y}
              r={GRAPH_RADIUS}
              fill={hl ? "#3b82f6" : "#4ade80"}
              stroke={hl ? "#93c5fd" : "#86efac"}
              strokeWidth={hl ? 2 : 1}
            />
            <text
              x={leftX - GRAPH_RADIUS - 4}
              y={y + 4}
              textAnchor="end"
              className="fill-[var(--text)]"
              style={{ fontSize: 10 }}
            >
              {f.id}
            </text>
          </g>
        );
      })}
      {caseList.map((c, j) => {
        const y = cy(j);
        const hl = highlightCase === c.name;
        const short = c.name.replace("TestLib/", "");
        return (
          <g
            key={c.name}
            role="button"
            tabIndex={0}
            aria-label={`${t("graphLegendCase")} ${c.name}`}
            style={{ cursor: "pointer" }}
            onClick={() => {
              onHighlightFeature(null);
              onHighlightCase(hl ? null : c.name);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onHighlightFeature(null);
                onHighlightCase(hl ? null : c.name);
              }
            }}
          >
            <circle
              cx={rightX}
              cy={y}
              r={GRAPH_RADIUS}
              fill={hl ? "#3b82f6" : "#60a5fa"}
              stroke={hl ? "#93c5fd" : "#93c5fd"}
              strokeWidth={hl ? 2 : 1}
            />
            <text
              x={rightX + GRAPH_RADIUS + 4}
              y={y + 4}
              textAnchor="start"
              className="fill-[var(--text)]"
              style={{ fontSize: 9 }}
            >
              {short.length > 18 ? short.slice(0, 16) + ".." : short}
            </text>
          </g>
        );
      })}
      <line
        x1={midX}
        y1={20}
        x2={midX}
        y2={svgHeight - 10}
        stroke="var(--text-muted)"
        strokeWidth={1}
        strokeDasharray="4,4"
        opacity={0.5}
      />
      <text x={leftX} y={14} textAnchor="middle" className="fill-[var(--text-muted)]" style={{ fontSize: 9 }}>
        {t("graphLegendFeature")}
      </text>
      <text x={rightX} y={14} textAnchor="middle" className="fill-[var(--text-muted)]" style={{ fontSize: 9 }}>
        {t("graphLegendCase")}
      </text>
    </svg>
  );
}
