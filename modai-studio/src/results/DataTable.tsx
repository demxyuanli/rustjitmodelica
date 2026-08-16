import { t } from "../i18n";

export function DataTable({
  time,
  columns,
  labels,
  series,
  cursorIndex,
}: {
  time: number[];
  columns: string[];
  labels: Record<string, string>;
  series: Record<string, number[]>;
  cursorIndex: number | null;
}) {
  if (!time.length || !columns.length) {
    return <div style={{ color: "var(--muted)" }}>{t("resultsNoSeries")}</div>;
  }
  return (
    <div className="data-table-wrap">
      <table className="data-table">
        <thead>
          <tr>
            <th>t</th>
            {columns.map((k) => (
              <th key={k} title={k}>
                {labels[k] ?? k}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {time.map((tv, i) => (
            <tr key={i} className={cursorIndex === i ? "cursor" : undefined}>
              <td>{tv}</td>
              {columns.map((k) => (
                <td key={k}>
                  {series[k]?.[i] != null ? Number(series[k]![i]).toPrecision(6) : ""}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
