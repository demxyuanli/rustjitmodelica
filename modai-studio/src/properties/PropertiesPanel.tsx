import { BookPlus, FileSpreadsheet, GitBranch, Pin, PinOff, Trash2 } from "lucide-react";
import type { CatalogEntry, DiagramNode, ParamDictionaryEntry } from "../api/types";
import { pickDataFile, readTextFile } from "../api/studio";
import { isDrawNode } from "../canvas/drawParams";
import { applyVariantToNode, variantOptionsFor } from "../catalog/variants";
import { samplePeriodLabel } from "../canvas/sampleTime";
import {
  activeVariantId,
  isVariantSubsystem,
  variantList,
} from "../canvas/variantOps";
import { t } from "../i18n";
import { IconButton } from "../shell/IconButton";
import { csvToModelicaTable } from "../util/csvToTable";

export function PropertiesPanel({
  node,
  catalog,
  dictionary,
  onChange,
  onDictionary,
  multiSelectHint,
  onAddVariant,
  onSetActiveVariant,
  onRenameVariant,
  onRemoveVariant,
}: {
  node: DiagramNode | null;
  catalog: CatalogEntry[];
  dictionary: ParamDictionaryEntry[];
  onChange: (n: DiagramNode) => void;
  onDictionary: (next: ParamDictionaryEntry[]) => void;
  multiSelectHint?: boolean;
  onAddVariant?: () => void;
  onSetActiveVariant?: (variantId: string) => void;
  onRenameVariant?: (variantId: string, label: string) => void;
  onRemoveVariant?: (variantId: string) => void;
}) {
  if (!node) {
    return (
      <div className="panel right">
        <h3>{t("properties")}</h3>
        <div className="hint" style={{ padding: 10 }}>
          {multiSelectHint ? t("selectionToolsHint") : t("selectBlock")}
        </div>
      </div>
    );
  }

  const draw = isDrawNode(node);
  const catalogEntry = catalog.find((c) => c.id === node.catalogId);
  const variants = draw ? null : variantOptionsFor(node.typeName, node.catalogId);
  const tsHint = samplePeriodLabel(node);
  const isFromFile =
    node.catalogId.toLowerCase().includes("fromfile") ||
    (node.catalogId.toLowerCase().includes("combitable") &&
      (node.params.some((p) => p.name === "fileName") ||
        node.params.some((p) => p.name === "table")));

  const promoteParam = (paramName: string, current: string) => {
    const base = `${node.instanceName}_${paramName}`.replace(/[^A-Za-z0-9_]/g, "_");
    let name = base;
    const taken = new Set(dictionary.map((d) => d.name));
    let i = 1;
    while (taken.has(name)) {
      name = `${base}_${i}`;
      i += 1;
    }
    const value = current.startsWith("$") ? "1" : current.trim() || "1";
    onDictionary([...dictionary, { name, value, description: `${node.instanceName}.${paramName}` }]);
    const params = node.params.map((p) =>
      p.name === paramName ? { ...p, value: `$${name}` } : p,
    );
    onChange({ ...node, params });
  };

  const loadCsv = async () => {
    const path = await pickDataFile();
    if (!path) return;
    try {
      const text = await readTextFile(path);
      const table = csvToModelicaTable(text);
      const escaped = path.replace(/\\/g, "/");
      const params = node.params.map((p) => {
        if (p.name === "table") return { ...p, value: table };
        if (p.name === "tableOnFile") return { ...p, value: "false" };
        if (p.name === "fileName") return { ...p, value: `"${escaped}"` };
        return p;
      });
      onChange({ ...node, params });
    } catch {
      /* status handled by caller if needed */
    }
  };

  return (
    <div className="panel right">
      <h3>{t("properties")}</h3>
      <div className="prop-row">
        <span>{t("propName")}</span>
        <input
          value={node.instanceName}
          onChange={(ev) =>
            onChange({
              ...node,
              instanceName: ev.target.value,
              ui: { ...node.ui, label: draw ? node.ui?.label : ev.target.value },
            })
          }
        />
      </div>
      <div className="prop-row">
        <span>{t("propType")}</span>
        {variants ? (
          <select
            value={node.catalogId}
            onChange={(ev) => {
              const next = variants.find((v) => v.catalogId === ev.target.value);
              if (!next) return;
              const entry = catalog.find((c) => c.id === next.catalogId);
              const applied = applyVariantToNode(node, next, entry);
              const isBar = (entry?.tags ?? []).includes("busbar");
              const fan = Math.max(
                applied.ports.filter((p) => p.direction === "in").length,
                applied.ports.filter((p) => p.direction === "out").length,
                1,
              );
              onChange({
                ...applied,
                ui: {
                  ...applied.ui,
                  ...(isBar
                    ? {
                        width: (entry?.tags ?? []).includes("bus") ? 28 : 22,
                        height: Math.max(48, 16 + fan * 18),
                      }
                    : {}),
                },
              });
            }}
          >
            {variants.map((v) => (
              <option key={v.catalogId} value={v.catalogId}>
                {v.label}
              </option>
            ))}
          </select>
        ) : (
          <span className="hint" style={{ wordBreak: "break-all" }}>
            {node.typeName}
          </span>
        )}
      </div>
      {draw ? (
        <>
          <div className="prop-row">
            <span>{t("propWidth")}</span>
            <input
              type="number"
              value={node.ui?.width ?? 140}
              onChange={(ev) =>
                onChange({
                  ...node,
                  ui: { ...node.ui, width: Number(ev.target.value) || 40 },
                })
              }
            />
          </div>
          <div className="prop-row">
            <span>{t("propHeight")}</span>
            <input
              type="number"
              value={node.ui?.height ?? 80}
              onChange={(ev) =>
                onChange({
                  ...node,
                  ui: { ...node.ui, height: Number(ev.target.value) || 16 },
                })
              }
            />
          </div>
        </>
      ) : null}
      {!draw ? (
        <div className="prop-row">
          <span>{t("propLabelBelow")}</span>
          <input
            type="checkbox"
            checked={!!node.ui?.labelBelow}
            onChange={(ev) =>
              onChange({
                ...node,
                ui: { ...node.ui, labelBelow: ev.target.checked },
              })
            }
          />
        </div>
      ) : null}
      {tsHint ? (
        <div className="prop-row">
          <span>{t("sampleTime")}</span>
          <span style={{ color: "var(--discrete)" }}>{tsHint}</span>
        </div>
      ) : null}
      {isFromFile ? (
        <div style={{ padding: "6px 10px" }}>
          <IconButton icon={FileSpreadsheet} labelKey="loadCsv" onClick={() => void loadCsv()} />
        </div>
      ) : null}
      {node.ui?.moAsset ? (
        <div className="prop-row">
          <span>{t("propRef")}</span>
          <span className="hint" style={{ wordBreak: "break-all" }}>
            {node.ui.moAsset}
          </span>
        </div>
      ) : null}
      {node.params.map((p, idx) => {
        const meta = catalogEntry?.defaultParams.find((d) => d.name === p.name);
        const unit = meta?.unit?.trim();
        const desc = meta?.description?.trim();
        return (
        <div key={p.name} style={{ borderBottom: "1px solid var(--border)", paddingBottom: 4 }}>
          <div className="prop-row">
            <span>
              {p.name}
              {unit ? <span className="prop-unit">{unit}</span> : null}
            </span>
            <input
              value={p.value}
              onChange={(ev) => {
                const params = node.params.map((x, i) =>
                  i === idx ? { ...x, value: ev.target.value } : x,
                );
                const next: DiagramNode = { ...node, params };
                if (draw && p.name === "text") {
                  next.ui = { ...node.ui, label: ev.target.value };
                }
                onChange(next);
              }}
            />
          </div>
          {desc ? <div className="prop-desc">{desc}</div> : null}
          {!draw ? (
            <div className="prop-actions">
              <IconButton icon={BookPlus} labelKey="promoteToDict" onClick={() => promoteParam(p.name, p.value)} />
              <IconButton
                icon={(node.ui?.dashboardPins ?? []).includes(p.name) ? PinOff : Pin}
                labelKey={(node.ui?.dashboardPins ?? []).includes(p.name) ? "unpinDashboard" : "pinDashboard"}
                onClick={() => {
                  const pins = new Set(node.ui?.dashboardPins ?? []);
                  if (pins.has(p.name)) pins.delete(p.name);
                  else pins.add(p.name);
                  onChange({
                    ...node,
                    ui: { ...node.ui, dashboardPins: [...pins] },
                  });
                }}
              />
              {p.value.trim().startsWith("$") ? (
                <span className="hint" style={{ marginLeft: 8 }}>
                  {t("dictRef")}
                </span>
              ) : null}
            </div>
          ) : null}
        </div>
        );
      })}
      {node.nodeKind === "subsystem" ? (
        <>
          <div className="prop-section">{t("variantSubsystem")}</div>
          <div className="hint" style={{ padding: "4px 10px" }}>
            {t("variantSubsystemHint")}
          </div>
          {isVariantSubsystem(node) ? (
            <>
              <div className="prop-row">
                <span>{t("activeVariant")}</span>
                <select
                  value={activeVariantId(node) ?? ""}
                  onChange={(ev) => onSetActiveVariant?.(ev.target.value)}
                >
                  {variantList(node).map((v) => (
                    <option key={v.id} value={v.id}>
                      {v.label}
                    </option>
                  ))}
                </select>
              </div>
              {variantList(node).map((v) => (
                <div key={v.id} style={{ padding: "4px 10px", borderBottom: "1px solid var(--border)" }}>
                  <div className="prop-row">
                    <span>{t("variantLabel")}</span>
                    <input
                      value={v.label}
                      onChange={(ev) => onRenameVariant?.(v.id, ev.target.value)}
                    />
                  </div>
                  {variantList(node).length > 1 ? (
                    <IconButton
                      icon={Trash2}
                      labelKey="variantRemove"
                      onClick={() => onRemoveVariant?.(v.id)}
                    />
                  ) : null}
                </div>
              ))}
            </>
          ) : null}
          <div style={{ padding: "6px 10px" }}>
            <IconButton icon={GitBranch} labelKey="addVariant" onClick={() => onAddVariant?.()} />
          </div>
        </>
      ) : null}
      {node.nodeKind === "subsystem" && node.ports.length > 0 ? (
        <>
          <div className="prop-section">{t("boundaryPorts")}</div>
          {node.ports.map((p) => {
            const bind = node.ui?.portBindings?.[p.id];
            return (
              <div className="prop-row" key={p.id}>
                <span className={p.direction === "in" ? "port-in" : "port-out"}>{p.id}</span>
                <span className="hint" style={{ wordBreak: "break-all" }}>
                  {bind ? `${bind.nodeId}.${bind.portId}` : p.direction}
                </span>
              </div>
            );
          })}
        </>
      ) : null}
    </div>
  );
}
