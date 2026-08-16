/** Convert CSV / whitespace table text into a Modelica CombiTimeTable `table=[...]` string. */
export function csvToModelicaTable(text: string): string {
  const rows: number[][] = [];
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith("#") || line.startsWith("//")) continue;
    const cells = line.includes(",")
      ? line.split(",").map((c) => c.trim())
      : line.split(/\s+/).map((c) => c.trim());
    if (!cells.length || cells.every((c) => !c)) continue;
    // Skip header row when first cell is non-numeric.
    if (rows.length === 0 && Number.isNaN(Number(cells[0]))) continue;
    const nums = cells.map((c) => Number(c)).filter((n) => Number.isFinite(n));
    if (nums.length >= 2) rows.push(nums);
  }
  if (!rows.length) {
    return "[0,0; 1,0]";
  }
  const body = rows.map((r) => r.join(",")).join("; ");
  return `[${body}]`;
}
