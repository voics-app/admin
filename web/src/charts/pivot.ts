export interface Row {
  t: string;
  [key: string]: string | number | undefined;
}

/** Merges labelled series into one row per timestamp so Recharts can plot them on a shared axis. */
export function pivotSeries(series: { label: string; points: [string, number][] }[]): Row[] {
  const rows = new Map<string, Row>();
  for (const { label, points } of series) {
    for (const [t, value] of points) {
      const row = rows.get(t) ?? { t };
      row[label] = value;
      rows.set(t, row);
    }
  }
  return [...rows.values()].sort((a, b) => Date.parse(a.t) - Date.parse(b.t));
}
