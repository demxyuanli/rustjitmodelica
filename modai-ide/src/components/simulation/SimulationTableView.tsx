import { useMemo, useState } from "react";
import { t } from "../../i18n";
import { AppIcon } from "../Icon";
import { IconButton } from "../IconButton";

interface SimulationTableViewProps {
  tableSortKey: string;
  tableSortAsc: boolean;
  tablePage: number;
  tablePageSize: number;
  visibleTableColumns: string[];
  tableColumns: string[];
  sortedTableRows: Record<string, number>[];
  onSortKeyChange: (value: string) => void;
  onSortAscChange: (value: boolean) => void;
  onPageChange: (value: number) => void;
  onPageSizeChange: (value: number) => void;
  onVisibleColumnsChange: (value: string[]) => void;
}

export function SimulationTableView({
  tableSortKey,
  tableSortAsc,
  tablePage,
  tablePageSize,
  visibleTableColumns,
  tableColumns,
  sortedTableRows,
  onSortKeyChange,
  onSortAscChange,
  onPageChange,
  onPageSizeChange,
  onVisibleColumnsChange,
}: SimulationTableViewProps) {
  const [showColumnsDropdown, setShowColumnsDropdown] = useState(false);

  const totalTablePages = sortedTableRows.length > 0 ? Math.ceil(sortedTableRows.length / tablePageSize) : 0;

  const paginatedRows = useMemo(
    () => sortedTableRows.slice(tablePage * tablePageSize, (tablePage + 1) * tablePageSize),
    [sortedTableRows, tablePage, tablePageSize]
  );

  const activeColumns = visibleTableColumns.length > 0 ? visibleTableColumns : tableColumns;

  const toggleTableColumn = (column: string) => {
    const next = visibleTableColumns.includes(column)
      ? visibleTableColumns.filter((item) => item !== column)
      : [...visibleTableColumns, column].sort((left, right) => tableColumns.indexOf(left) - tableColumns.indexOf(right));
    onVisibleColumnsChange(next);
  };

  if (tableColumns.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[var(--text-muted)]">
        {t("runSimulationToSeePlot")}
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 min-w-0 w-full max-w-full flex-1 flex-col overflow-hidden">
      <div className="flex shrink-0 items-center border-b border-border bg-surface">
        {/* Scrollable pagination section */}
        <div className="min-w-0 flex-1 overflow-x-auto scroll-vscode px-2 py-1">
          <div className="flex min-w-max items-center gap-2">
            <span className="text-[11px] text-[var(--text-muted)]">{t("tablePageSize")}</span>
            <select
              value={tablePageSize}
              onChange={(event) => {
                onPageSizeChange(Number(event.target.value));
                onPageChange(0);
              }}
              className="h-7 w-16 rounded border border-border bg-surface px-1 py-0 text-[11px]"
            >
              <option value={50}>50</option>
              <option value={100}>100</option>
              <option value={200}>200</option>
              <option value={500}>500</option>
            </select>

            <IconButton
              icon={<AppIcon name="prev" aria-hidden="true" />}
              size="xs"
              className="border theme-button-secondary"
              disabled={tablePage <= 0}
              onClick={() => onPageChange(Math.max(0, tablePage - 1))}
              title={t("previousPage")}
              aria-label={t("previousPage")}
            />
            <span className="min-w-16 text-center text-[11px] text-[var(--text-muted)] tabular-nums">
              {tablePage + 1} / {totalTablePages || 1}
            </span>
            <IconButton
              icon={<AppIcon name="next" aria-hidden="true" />}
              size="xs"
              className="border theme-button-secondary"
              disabled={tablePage >= totalTablePages - 1}
              onClick={() => onPageChange(Math.min(totalTablePages - 1, tablePage + 1))}
              title={t("nextPage")}
              aria-label={t("nextPage")}
            />
          </div>
        </div>

        {/* Columns button — outside the overflow-x-auto wrapper so dropdown is not clipped */}
        <div className="relative shrink-0 border-l border-border px-2 py-1">
          <IconButton
            icon={<AppIcon name="columns" aria-hidden="true" />}
            size="xs"
            className="border theme-button-secondary"
            onClick={() => setShowColumnsDropdown((value) => !value)}
            title={t("columnsSelect")}
            aria-label={t("columnsSelect")}
          />
          {showColumnsDropdown && (
            <div
              className="absolute right-0 top-full z-30 mt-1 max-h-56 min-w-36 overflow-auto rounded border border-border p-1 shadow-xl scroll-vscode"
              style={{ backgroundColor: "var(--surface-alt, #2a2d36)" }}
            >
              {tableColumns.map((column) => (
                <label key={column} className="flex cursor-pointer items-center gap-2 whitespace-nowrap px-2 py-1 text-[11px]">
                  <input
                    type="checkbox"
                    checked={visibleTableColumns.includes(column)}
                    onChange={() => toggleTableColumn(column)}
                  />
                  <span>{column}</span>
                </label>
              ))}
              <button
                type="button"
                className="mt-1 w-full rounded border border-border px-2 py-1 text-[11px] theme-button-secondary"
                onClick={() => setShowColumnsDropdown(false)}
              >
                {t("closeTab")}
              </button>
            </div>
          )}
        </div>
      </div>

      <div
        className="min-h-0 min-w-0 w-full max-w-full flex-1 overflow-auto scroll-vscode"
        style={{ scrollbarGutter: "stable both-edges" }}
      >
        <div className="inline-block min-w-full align-top">
          <table className="min-w-max border-collapse text-[11px] leading-tight">
            <thead className="z-10 bg-surface-alt shadow-[0_1px_0_0_var(--border)]">
              <tr>
                {activeColumns.map((column) => (
                  <th
                    key={column}
                    className="sticky top-0 z-20 cursor-pointer whitespace-nowrap border border-border px-1.5 py-0.5 text-left font-medium hover:bg-[var(--surface-hover)]"
                    style={{ backgroundColor: "var(--surface-alt, #2a2d36)" }}
                    onClick={() => {
                      onSortKeyChange(column);
                      onSortAscChange(tableSortKey === column ? !tableSortAsc : true);
                    }}
                  >
                    {column} {tableSortKey === column ? (tableSortAsc ? "\u2191" : "\u2193") : ""}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {paginatedRows.map((row, rowIndex) => (
                <tr key={tablePage * tablePageSize + rowIndex}>
                  {activeColumns.map((column) => (
                    <td key={column} className="whitespace-nowrap border border-border px-1.5 py-0.5 font-mono tabular-nums">
                      {typeof row[column] === "number" ? row[column].toExponential(4) : row[column]}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
