import { useState, useCallback } from "react";
import { Plus, Trash2 } from "lucide-react";
import { t } from "../../i18n";

export interface VariableDecl {
  name: string;
  typeName: string;
  variability: "variable" | "parameter" | "constant";
  startValue: string;
  unit: string;
  description: string;
}

interface VariableDeclarationPanelProps {
  variables: VariableDecl[];
  readOnly?: boolean;
  onChange: (variables: VariableDecl[]) => void;
}

const TYPE_OPTIONS = ["Real", "Integer", "Boolean", "String"];
const VARIABILITY_OPTIONS: VariableDecl["variability"][] = ["variable", "parameter", "constant"];

function emptyVariable(): VariableDecl {
  return { name: "", typeName: "Real", variability: "variable", startValue: "", unit: "", description: "" };
}

export function VariableDeclarationPanel({
  variables,
  readOnly = false,
  onChange,
}: VariableDeclarationPanelProps) {
  const [editIdx, setEditIdx] = useState<number | null>(null);

  const handleAdd = useCallback(() => {
    onChange([...variables, emptyVariable()]);
    setEditIdx(variables.length);
  }, [variables, onChange]);

  const handleDelete = useCallback(
    (idx: number) => {
      onChange(variables.filter((_, i) => i !== idx));
      if (editIdx === idx) setEditIdx(null);
    },
    [variables, onChange, editIdx],
  );

  const handleUpdate = useCallback(
    (idx: number, field: keyof VariableDecl, value: string) => {
      const next = variables.map((v, i) =>
        i === idx ? { ...v, [field]: value } : v,
      );
      onChange(next);
    },
    [variables, onChange],
  );

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between px-2 py-1">
        <span className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide">
          {t("variableDeclarations")}
        </span>
        {!readOnly && (
          <button
            type="button"
            className="p-1 rounded bg-primary/20 text-primary hover:bg-primary/30"
            onClick={handleAdd}
            title={t("addVariable")}
          >
            <Plus className="h-4 w-4" />
          </button>
        )}
      </div>
      <div className="overflow-auto max-h-[300px]">
        <table className="w-full text-[10px] border-collapse">
          <thead>
            <tr className="text-[var(--text-muted)] border-b border-[var(--border)]">
              <th className="text-left px-1.5 py-1 font-medium">{t("variableName")}</th>
              <th className="text-left px-1.5 py-1 font-medium">{t("variableType")}</th>
              <th className="text-left px-1.5 py-1 font-medium">{t("variableVariability")}</th>
              <th className="text-left px-1.5 py-1 font-medium">{t("variableStart")}</th>
              <th className="text-left px-1.5 py-1 font-medium">{t("variableUnit")}</th>
              <th className="text-left px-1.5 py-1 font-medium">{t("variableDesc")}</th>
              {!readOnly && <th className="w-6" />}
            </tr>
          </thead>
          <tbody>
            {variables.map((v, idx) => {
              const editing = editIdx === idx;
              return (
                <tr
                  key={idx}
                  className={`border-b border-[var(--border)]/30 hover:bg-white/5 ${editing ? "bg-primary/5" : ""}`}
                  onClick={() => !readOnly && setEditIdx(idx)}
                >
                  <td className="px-1.5 py-0.5">
                    {editing && !readOnly ? (
                      <input
                        className="w-full bg-transparent border-b border-primary text-[var(--text)] text-[10px] outline-none"
                        value={v.name}
                        onChange={(e) => handleUpdate(idx, "name", e.target.value)}
                        autoFocus
                      />
                    ) : (
                      <span className="text-[var(--text)]">{v.name || "-"}</span>
                    )}
                  </td>
                  <td className="px-1.5 py-0.5">
                    {editing && !readOnly ? (
                      <select
                        className="bg-transparent text-[var(--text)] text-[10px] outline-none"
                        value={v.typeName}
                        onChange={(e) => handleUpdate(idx, "typeName", e.target.value)}
                      >
                        {TYPE_OPTIONS.map((opt) => (
                          <option key={opt} value={opt}>{opt}</option>
                        ))}
                      </select>
                    ) : (
                      <span className="text-[var(--text-muted)]">{v.typeName}</span>
                    )}
                  </td>
                  <td className="px-1.5 py-0.5">
                    {editing && !readOnly ? (
                      <select
                        className="bg-transparent text-[var(--text)] text-[10px] outline-none"
                        value={v.variability}
                        onChange={(e) => handleUpdate(idx, "variability", e.target.value)}
                      >
                        {VARIABILITY_OPTIONS.map((opt) => (
                          <option key={opt} value={opt}>{opt}</option>
                        ))}
                      </select>
                    ) : (
                      <span className="text-[var(--text-muted)]">{v.variability}</span>
                    )}
                  </td>
                  <td className="px-1.5 py-0.5">
                    {editing && !readOnly ? (
                      <input
                        className="w-full bg-transparent border-b border-primary text-[var(--text)] text-[10px] outline-none font-mono"
                        value={v.startValue}
                        onChange={(e) => handleUpdate(idx, "startValue", e.target.value)}
                      />
                    ) : (
                      <span className="text-[var(--text)] font-mono">{v.startValue || "-"}</span>
                    )}
                  </td>
                  <td className="px-1.5 py-0.5">
                    {editing && !readOnly ? (
                      <input
                        className="w-16 bg-transparent border-b border-primary text-[var(--text)] text-[10px] outline-none"
                        value={v.unit}
                        onChange={(e) => handleUpdate(idx, "unit", e.target.value)}
                      />
                    ) : (
                      <span className="text-[var(--text-muted)]">{v.unit || "-"}</span>
                    )}
                  </td>
                  <td className="px-1.5 py-0.5">
                    {editing && !readOnly ? (
                      <input
                        className="w-full bg-transparent border-b border-primary text-[var(--text)] text-[10px] outline-none"
                        value={v.description}
                        onChange={(e) => handleUpdate(idx, "description", e.target.value)}
                      />
                    ) : (
                      <span className="text-[var(--text-muted)] truncate block max-w-[120px]">{v.description || "-"}</span>
                    )}
                  </td>
                  {!readOnly && (
                    <td className="px-0.5 py-0.5">
                      <button
                        type="button"
                        className="p-0.5 text-red-400 hover:text-red-300"
                        onClick={(e) => { e.stopPropagation(); handleDelete(idx); }}
                        title={t("deleteVariable")}
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </button>
                    </td>
                  )}
                </tr>
              );
            })}
            {variables.length === 0 && (
              <tr>
                <td colSpan={readOnly ? 6 : 7} className="text-center py-3 text-[var(--text-muted)]">
                  {readOnly ? "No variables" : "Click + to add variables"}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
