import { useMemo, useState } from "react";
import { api } from "../api/client";
import type { HealthPanel, Range, ServiceStatus } from "../api/types";
import { useAsync } from "../api/useAsync";
import { assignSlots, foldOverflow } from "../charts/palette";
import { pivotSeries } from "../charts/pivot";
import { TimeSeriesChart } from "../charts/TimeSeriesChart";
import { ErrorBanner, Loaded, PageHead, RangePicker, StatusBadge } from "../components/common";
import { formatCount, formatMilliseconds, formatPercent, formatRate } from "../format";

const STATUS: Record<ServiceStatus, { tone: "good" | "warning" | "critical"; label: string }> = {
  healthy: { tone: "good", label: "Healthy" },
  degraded: { tone: "warning", label: "Degraded" },
  down: { tone: "critical", label: "Down" },
};

const PANELS: { panel: HealthPanel; title: string; subtitle: string; format: (v: number) => string }[] = [
  { panel: "request_rate", title: "Request rate", subtitle: "Requests per second, by service", format: formatRate },
  { panel: "error_ratio", title: "Error ratio", subtitle: "Share of 5xx responses, by service", format: (v) => formatPercent(v) },
  { panel: "latency_p95", title: "Latency p95", subtitle: "By service", format: formatMilliseconds },
  { panel: "consumer_lag", title: "Consumer lag", subtitle: "Messages behind, by consumer group", format: formatCount },
  { panel: "db_latency_p95", title: "Database latency p95", subtitle: "By database", format: formatMilliseconds },
  { panel: "gateway_sessions", title: "Gateway sessions", subtitle: "Online sessions, by node", format: formatCount },
];

export function PlatformHealthPage() {
  const [range, setRange] = useState<Range>("1h");
  const state = useAsync(api.platformHealth, [], 15_000);
  return (
    <div className="stack">
      <PageHead title="Platform health" subtitle="Service availability and golden signals from VictoriaMetrics.">
        <RangePicker value={range} onChange={setRange} />
      </PageHead>
      <Loaded state={state}>
        {(health) => (
          <>
            {!health.metrics_reachable ? (
              <div className="banner warning" role="status">
                VictoriaMetrics is unreachable. Service status below cannot be confirmed.
              </div>
            ) : null}
            <section className="panel" aria-label="Services">
              <div className="table-wrap">
                <table>
                  <thead>
                    <tr>
                      <th>Service</th>
                      <th>Status</th>
                      <th className="num">Instances</th>
                      <th className="num">Requests</th>
                      <th className="num">Errors</th>
                      <th className="num">Latency p95</th>
                    </tr>
                  </thead>
                  <tbody>
                    {health.services.map((s) => (
                      <tr key={s.service}>
                        <td>{s.service}</td>
                        <td>
                          <StatusBadge {...STATUS[s.status]} />
                        </td>
                        <td className="num">
                          {s.instances_up} / {s.instances_total}
                        </td>
                        <td className="num">{formatRate(s.request_rate)}</td>
                        <td className="num">{formatPercent(s.error_ratio)}</td>
                        <td className="num">{formatMilliseconds(s.latency_p95_ms)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </section>
          </>
        )}
      </Loaded>
      <div className="grid wide">
        {PANELS.map((p) => (
          <HealthChart key={p.panel} {...p} range={range} />
        ))}
      </div>
    </div>
  );
}

function HealthChart({ panel, title, subtitle, format, range }: (typeof PANELS)[number] & { range: Range }) {
  const state = useAsync(() => api.healthSeries(panel, range), [panel, range], 30_000);
  const prepared = useMemo(() => {
    if (!state.data) return undefined;
    const { kept, folded } = foldOverflow(state.data.series);
    const specs = assignSlots(kept.map((s) => s.label)).map((a) => ({ key: a.label, label: a.label, slot: a.slot }));
    return { rows: pivotSeries(kept), specs, folded };
  }, [state.data]);

  if (state.error && !prepared) return <ErrorBanner error={state.error} onRetry={state.reload} />;
  if (!prepared) return <div className="panel empty">Loading {title.toLowerCase()}...</div>;
  return (
    <TimeSeriesChart
      title={title}
      subtitle={subtitle}
      kind="line"
      rows={prepared.rows}
      series={prepared.specs}
      range={range}
      format={format}
      folded={prepared.folded}
    />
  );
}
