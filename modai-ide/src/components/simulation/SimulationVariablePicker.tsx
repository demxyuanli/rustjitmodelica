import { useMemo, useState } from "react";
import { t } from "../../i18n";
import { AppIcon } from "../Icon";
import { IconButton } from "../IconButton";

interface SimulationVariablePickerProps {
  variableNames: string[];
  selectedNames: string[];
  onToggleVariable: (name: string) => void;
  onSelectAll: () => void;
  onClearAll: () => void;
}

export function SimulationVariablePicker({
  variableNames,
  selectedNames,
  onToggleVariable,
  onSelectAll,
  onClearAll,
}: SimulationVariablePickerProps) {
  const [filterText, setFilterText] = useState("");

  const filteredNames = useMemo(() => {
    const keyword = filterText.trim().toLowerCase();
    if (!keyword) return variableNames;
    return variableNames.filter((name) => name.toLowerCase().includes(keyword));
  }, [filterText, variableNames]);

  return (
    <aside className="w-56 shrink-0 border-r border-border bg-surface-alt/60 p-2 text-xs">
      <div className="mb-2 flex items-center justify-between gap-2">
        <div>
          <div className="font-medium text-[var(--text)]">{t("variablesSelect")}</div>
          <div className="text-[11px] text-[var(--text-muted)]">
            {selectedNames.length} / {variableNames.length}
          </div>
        </div>
        <div className="flex gap-1">
          <IconButton
            icon={<AppIcon name="stage" aria-hidden="true" />}
            size="xs"
            onClick={onSelectAll}
            title={t("selectAllVariables")}
            aria-label={t("selectAllVariables")}
            disabled={variableNames.length === 0}
          />
          <IconButton
            icon={<AppIcon name="unstage" aria-hidden="true" />}
            size="xs"
            onClick={onClearAll}
            title={t("clearVariableSelection")}
            aria-label={t("clearVariableSelection")}
            disabled={selectedNames.length === 0}
          />
        </div>
      </div>

      <input
        type="text"
        value={filterText}
        onChange={(event) => setFilterText(event.target.value)}
        placeholder={t("tableSearch")}
        className="mb-2 w-full rounded border border-border bg-surface px-2 py-1 text-xs text-[var(--text)]"
      />

      {variableNames.length === 0 ? (
        <div className="pt-2 text-[var(--text-muted)]">{t("runJitFirst")}</div>
      ) : filteredNames.length === 0 ? (
        <div className="pt-2 text-[var(--text-muted)]">{t("noSearchResults")}</div>
      ) : (
        <div className="max-h-full space-y-1 overflow-auto pr-1 scroll-vscode">
          {filteredNames.map((name) => (
            <label
              key={name}
              className="flex cursor-pointer items-center gap-2 rounded px-2 py-1 hover:bg-[var(--surface-hover)]"
              title={name}
            >
              <input
                type="checkbox"
                checked={selectedNames.includes(name)}
                onChange={() => onToggleVariable(name)}
                className="shrink-0"
              />
              <span className="truncate font-mono text-[11px]">{name}</span>
            </label>
          ))}
        </div>
      )}
    </aside>
  );
}
