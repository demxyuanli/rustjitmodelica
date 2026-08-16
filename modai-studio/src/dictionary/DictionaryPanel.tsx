import type { ParamDictionaryEntry, StudioProjectManifest } from "../api/types";
import { Plus, Trash2 } from "lucide-react";
import { t } from "../i18n";
import { IconButton } from "../shell/IconButton";

export function DictionaryPanel({
  manifest,
  onManifest,
  onLoggedVars,
}: {
  manifest: StudioProjectManifest;
  onManifest: (m: StudioProjectManifest) => void;
  onLoggedVars?: (vars: string[]) => void;
}) {
  const dict = manifest.paramDictionary ?? [];
  const logged = (manifest.simulationDefaults.loggedVariables ?? []).join(", ");

  const setDict = (next: ParamDictionaryEntry[]) => {
    onManifest({ ...manifest, paramDictionary: next });
  };

  const updateRow = (idx: number, patch: Partial<ParamDictionaryEntry>) => {
    setDict(dict.map((e, i) => (i === idx ? { ...e, ...patch } : e)));
  };

  const addRow = () => {
    let n = 1;
    const names = new Set(dict.map((e) => e.name));
    while (names.has(`P${n}`)) n += 1;
    setDict([...dict, { name: `P${n}`, value: "1", description: "" }]);
  };

  const removeRow = (idx: number) => {
    setDict(dict.filter((_, i) => i !== idx));
  };

  return (
    <div className="panel right">
      <h3>{t("paramDictionary")}</h3>
      <div className="hint" style={{ padding: "4px 10px" }}>
        {t("paramDictionaryHint")}
      </div>
      {dict.map((e, idx) => (
        <div key={`${e.name}_${idx}`} style={{ padding: "6px 10px", borderBottom: "1px solid var(--border)" }}>
          <div className="prop-row">
            <span>{t("dictName")}</span>
            <input value={e.name} onChange={(ev) => updateRow(idx, { name: ev.target.value })} />
          </div>
          <div className="prop-row">
            <span>{t("dictValue")}</span>
            <input value={e.value} onChange={(ev) => updateRow(idx, { value: ev.target.value })} />
          </div>
          <div className="prop-row">
            <span>{t("dictDesc")}</span>
            <input
              value={e.description ?? ""}
              onChange={(ev) => updateRow(idx, { description: ev.target.value })}
            />
          </div>
          <IconButton icon={Trash2} labelKey="dictRemove" onClick={() => removeRow(idx)} />
        </div>
      ))}
      <div style={{ padding: 10 }}>
        <IconButton icon={Plus} labelKey="dictAdd" onClick={addRow} />
      </div>
      <div className="prop-section">{t("loggedVariables")}</div>
      <div className="hint" style={{ padding: "4px 10px" }}>
        {t("loggedVariablesHint")}
      </div>
      <div className="prop-row">
        <span>vars</span>
        <input
          value={logged}
          placeholder="gain.y, plant.y"
          onChange={(ev) => {
            const vars = ev.target.value
              .split(/[,;\s]+/)
              .map((s) => s.trim())
              .filter(Boolean);
            onManifest({
              ...manifest,
              simulationDefaults: {
                ...manifest.simulationDefaults,
                loggedVariables: vars,
              },
            });
            onLoggedVars?.(vars);
          }}
        />
      </div>
    </div>
  );
}
