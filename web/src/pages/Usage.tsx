import { useMemo, useState } from "react";
import { api } from "../api/client";
import type { Range, Usage } from "../api/types";
import { useAsync } from "../api/useAsync";
import { DELIVERY_SLOT } from "../charts/palette";
import type { Row } from "../charts/pivot";
import { TimeSeriesChart } from "../charts/TimeSeriesChart";
import { Loaded, PageHead, RangePicker } from "../components/common";
import { formatCompact, formatMinutes } from "../format";

const count = (value: number) => formatCompact(value);

function usageRows(usage: Usage): Row[] {
  return usage.points.map((p) => ({ ...p }));
}

function activeUserRows(usage: Usage): Row[] {
  return usage.active_users.map((a) => ({ t: `${a.day}T00:00:00Z`, dau: a.dau, mau: a.mau }));
}

export function UsagePage() {
  const [range, setRange] = useState<Range>("24h");
  const state = useAsync(() => api.usage(range), [range], 60_000);
  return (
    <div className="stack">
      <PageHead title="Usage" subtitle="Counters aggregated from platform events into minute, hour and day rollups.">
        <RangePicker value={range} onChange={setRange} />
      </PageHead>
      <Loaded state={state}>{(usage) => <UsageCharts usage={usage} range={range} />}</Loaded>
    </div>
  );
}

function UsageCharts({ usage, range }: { usage: Usage; range: Range }) {
  const rows = useMemo(() => usageRows(usage), [usage]);
  const active = useMemo(() => activeUserRows(usage), [usage]);
  return (
    <div className="grid wide">
      <TimeSeriesChart
        title="Messages"
        subtitle={`Per ${usage.granularity}, by delivery mode`}
        kind="stackedBar"
        rows={rows}
        range={range}
        format={count}
        series={[
          { key: "messages_standard", label: "Standard", slot: DELIVERY_SLOT.standard },
          { key: "messages_ephemeral", label: "Ephemeral (E2EE)", slot: DELIVERY_SLOT.ephemeral },
          { key: "messages_p2p", label: "P2P", slot: DELIVERY_SLOT.p2p },
        ]}
      />
      <TimeSeriesChart
        title="Voice minutes"
        subtitle={`Per ${usage.granularity}`}
        kind="area"
        rows={rows}
        range={range}
        format={formatMinutes}
        series={[{ key: "voice_minutes", label: "Voice minutes", slot: 0 }]}
      />
      <TimeSeriesChart
        title="Peak sessions online"
        kind="line"
        rows={rows}
        range={range}
        format={count}
        series={[{ key: "sessions_peak", label: "Sessions", slot: 0 }]}
      />
      <TimeSeriesChart
        title="Peak active calls"
        kind="line"
        rows={rows}
        range={range}
        format={count}
        series={[{ key: "active_calls_peak", label: "Calls", slot: 2 }]}
      />
      <TimeSeriesChart
        title="Signups"
        kind="stackedBar"
        rows={rows}
        range={range}
        format={count}
        series={[{ key: "signups", label: "Signups", slot: 0 }]}
      />
      <TimeSeriesChart
        title="Daily and monthly active users"
        kind="line"
        rows={active}
        range="30d"
        format={count}
        series={[
          { key: "dau", label: "DAU", slot: 0 },
          { key: "mau", label: "MAU", slot: 1 },
        ]}
      />
    </div>
  );
}
