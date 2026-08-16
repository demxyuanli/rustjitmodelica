import type { CatalogEntry, DiagramNode, ParamDictionaryEntry } from "../api/types";
import { Play } from "lucide-react";
import { t } from "../i18n";
import { IconButton } from "../shell/IconButton";

function isNumeric(value: string): boolean {
  return /^-?\d+(\.\d+)?([eE][-+]?\d+)?$/.test(value.trim());
}

export function DashboardPanel({
  nodes,
  catalog,
  dictionary,
  onChangeParam,
  onChangeDict,
  onApplyRun,
  busy,
}: {
  nodes: DiagramNode[];
  catalog: CatalogEntry[];
  dictionary: ParamDictionaryEntry[];
  onChangeParam: (nodeId: string, paramName: string, value: string) => void;
  onChangeDict: (name: string, value: string) => void;
  onApplyRun?: () => void;
  busy?: boolean;
}) {
  const knobs = nodes.flatMap((n) =>
    (n.ui?.dashboardPins ?? [])
      .map((name) => n.params.find((p) => p.name === name))
      .filter((p): p is NonNullable<typeof p> => !!p && isNumeric(p.value))
      .map((p) => ({ node: n, param: p })),
  );
  const dictKnobs = dictionary.filter((e) => isNumeric(e.value));

  return (
    <div className="panel right" style={{ borderLeft: "none", borderTop: "1px solid var(--border)" }}>
      <h3>{t("dashboard")}</h3>
      <div className="hint" style={{ padding: "4px 10px" }}>
        {t("dashboardPinnedHint")}
      </div>
      {onApplyRun ? (
        <div style={{ padding: "4px 10px" }}>
          <IconButton icon={Play} labelKey="applyAndRun" primary disabled={busy} onClick={onApplyRun} />
        </div>
      ) : null}
      {dictKnobs.length ? (
        <>
          <div className="prop-section">{t("paramDictionary")}</div>
          {dictKnobs.map((e) => {
            const num = Number(e.value);
            const lo = Number.isFinite(num) ? Math.min(0, num * 0.2) : 0;
            const hi = Number.isFinite(num) ? Math.max(num * 2, 1) : 10;
            return (
              <div key={`dict_${e.name}`} style={{ padding: "6px 10px" }}>
                <div>
                  ${e.name} = {e.value}
                </div>
                <input
                  type="range"
                  min={lo}
                  max={hi || 1}
                  step={(hi - lo) / 100 || 0.01}
                  value={Number.isFinite(num) ? num : 0}
                  style={{ width: "100%" }}
                  onChange={(ev) => onChangeDict(e.name, ev.target.value)}
                />
              </div>
            );
          })}
        </>
      ) : null}
      {!knobs.length && !dictKnobs.length ? (
        <div className="hint" style={{ padding: 10 }}>{t("noPinnedParams")}</div>
      ) : (
        knobs.map(({ node, param }) => {
          const num = Number(param.value);
          const lo = Number.isFinite(num) ? Math.min(0, num * 0.2) : 0;
          const hi = Number.isFinite(num) ? Math.max(num * 2, 1) : 10;
          const unit = catalog
            .find((c) => c.id === node.catalogId)
            ?.defaultParams.find((d) => d.name === param.name)
            ?.unit?.trim();
          return (
            <div key={`${node.id}_${param.name}`} style={{ padding: "6px 10px" }}>
              <div>
                {node.instanceName}.{param.name}
                {unit ? <span className="prop-unit">{unit}</span> : null} = {param.value}
              </div>
              <input
                type="range"
                min={lo}
                max={hi || 1}
                step={(hi - lo) / 100 || 0.01}
                value={Number.isFinite(num) ? num : 0}
                style={{ width: "100%" }}
                onChange={(ev) => onChangeParam(node.id, param.name, ev.target.value)}
              />
            </div>
          );
        })
      )}
    </div>
  );
}
