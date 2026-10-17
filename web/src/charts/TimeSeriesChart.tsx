import { useId, useState } from "react";
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { formatTick, formatTimestamp } from "../format";
import { seriesColor } from "./palette";
import type { Row } from "./pivot";

export interface SeriesSpec {
  key: string;
  label: string;
  slot: number;
}

interface Props {
  title: string;
  subtitle?: string;
  kind: "line" | "area" | "stackedBar";
  rows: Row[];
  series: SeriesSpec[];
  range: string;
  format: (value: number) => string;
  /** Series that were folded into "Other" because the palette has a fixed number of hues. */
  folded?: number;
}

const AXIS = { stroke: "var(--axis)" };

interface TooltipPayload {
  dataKey?: string | number;
  value?: number | string;
}

function ChartTooltip({
  active,
  payload,
  label,
  series,
  format,
}: {
  active?: boolean;
  payload?: TooltipPayload[];
  label?: string | number;
  series: SeriesSpec[];
  format: (value: number) => string;
}) {
  if (!active || !payload?.length) return null;
  return (
    <div className="tooltip">
      <div className="muted">{formatTimestamp(String(label))}</div>
      {series.map((s) => {
        const entry = payload.find((p) => p.dataKey === s.key);
        if (entry?.value == null) return null;
        return (
          <div className="row" key={s.key}>
            <span className="legend-item">
              <span className="swatch" style={{ background: seriesColor(s.slot) }} />
              {s.label}
            </span>
            <strong>{format(Number(entry.value))}</strong>
          </div>
        );
      })}
    </div>
  );
}

/**
 * A time-series chart with the dataviz defaults: one axis, thin marks, hairline grid, a legend for two or more
 * series, a crosshair tooltip, and a table view so no value is colour-only.
 */
export function TimeSeriesChart({ title, subtitle, kind, rows, series, range, format, folded = 0 }: Props) {
  const [view, setView] = useState<"chart" | "table">("chart");
  const headingId = useId();
  const tick = (value: string) => formatTick(value, range);
  const tooltip = (
    <Tooltip
      cursor={{ stroke: "var(--axis)", strokeWidth: 1 }}
      content={<ChartTooltip series={series} format={format} />}
    />
  );
  const axes = (
    <>
      <CartesianGrid vertical={false} />
      <XAxis dataKey="t" tickFormatter={tick} tickLine={false} axisLine={AXIS} minTickGap={36} />
      <YAxis tickFormatter={(v: number) => format(v)} tickLine={false} axisLine={false} width={64} />
      {tooltip}
    </>
  );

  return (
    <section className="panel" aria-labelledby={headingId}>
      <div className="panel-head">
        <div>
          <h3 id={headingId}>{title}</h3>
          {subtitle ? <p>{subtitle}</p> : null}
        </div>
        <div className="segmented" role="group" aria-label={`${title} view`}>
          <button type="button" aria-pressed={view === "chart"} onClick={() => setView("chart")}>
            Chart
          </button>
          <button type="button" aria-pressed={view === "table"} onClick={() => setView("table")}>
            Table
          </button>
        </div>
      </div>

      {rows.length === 0 ? (
        <div className="empty">No data in this range yet.</div>
      ) : view === "table" ? (
        <div className="table-wrap" style={{ maxHeight: 260, overflowY: "auto" }}>
          <table>
            <thead>
              <tr>
                <th>Time</th>
                {series.map((s) => (
                  <th key={s.key} className="num">
                    {s.label}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.t}>
                  <td>{formatTimestamp(row.t)}</td>
                  {series.map((s) => (
                    <td key={s.key} className="num">
                      {typeof row[s.key] === "number" ? format(row[s.key] as number) : "n/a"}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="chart">
          <ResponsiveContainer width="100%" height="100%">
            {kind === "stackedBar" ? (
              <BarChart data={rows} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
                {axes}
                {series.map((s, index) => (
                  <Bar
                    key={s.key}
                    dataKey={s.key}
                    stackId="total"
                    fill={seriesColor(s.slot)}
                    stroke="var(--panel)"
                    strokeWidth={2}
                    radius={index === series.length - 1 ? [4, 4, 0, 0] : 0}
                    isAnimationActive={false}
                  />
                ))}
              </BarChart>
            ) : kind === "area" ? (
              <AreaChart data={rows} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
                {axes}
                {series.map((s) => (
                  <Area
                    key={s.key}
                    dataKey={s.key}
                    type="monotone"
                    stroke={seriesColor(s.slot)}
                    strokeWidth={2}
                    fill={seriesColor(s.slot)}
                    fillOpacity={0.14}
                    dot={false}
                    isAnimationActive={false}
                  />
                ))}
              </AreaChart>
            ) : (
              <LineChart data={rows} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
                {axes}
                {series.map((s) => (
                  <Line
                    key={s.key}
                    dataKey={s.key}
                    type="monotone"
                    stroke={seriesColor(s.slot)}
                    strokeWidth={2}
                    dot={false}
                    activeDot={{ r: 4, stroke: "var(--panel)", strokeWidth: 2 }}
                    connectNulls
                    isAnimationActive={false}
                  />
                ))}
              </LineChart>
            )}
          </ResponsiveContainer>
        </div>
      )}

      {series.length >= 2 ? (
        <ul className="legend" aria-label={`${title} legend`} style={{ listStyle: "none", padding: 0 }}>
          {series.map((s) => (
            <li className="legend-item" key={s.key}>
              <span className="swatch" style={{ background: seriesColor(s.slot) }} />
              {s.label}
            </li>
          ))}
          {folded > 0 ? <li className="muted">+{folded} more not shown</li> : null}
        </ul>
      ) : null}
    </section>
  );
}
