import { api } from "../api/client";
import { useAsync } from "../api/useAsync";
import { Loaded, PageHead, StatTile } from "../components/common";
import { formatCount, formatMinutes, formatTimestamp } from "../format";

const SOURCE_LABELS: Record<string, string> = {
  api: "platform totals (api)",
  gateway: "gateway stats",
  rollups: "usage rollups",
  voice: "voice directory (etcd)",
};

export function OverviewPage() {
  const state = useAsync(api.overview, [], 30_000);
  return (
    <div className="stack">
      <PageHead title="Overview" subtitle="Platform totals and the last 24 hours at a glance." />
      <Loaded state={state}>
        {(o) => (
          <>
            {o.degraded.length > 0 ? (
              <div className="banner warning" role="status">
                Some sources are unavailable: {o.degraded.map((s) => SOURCE_LABELS[s] ?? s).join(", ")}. Affected tiles show n/a.
              </div>
            ) : null}
            <section aria-label="Platform totals" className="grid">
              <StatTile label="Users" value={formatCount(o.platform?.users_total)} hint={`${formatCount(o.platform?.signups_24h)} signups in 24 h`} />
              <StatTile label="Guilds" value={formatCount(o.platform?.guilds_total)} />
              <StatTile label="Channels" value={formatCount(o.platform?.channels_total)} />
              <StatTile label="Bots" value={formatCount(o.platform?.bots_total)} />
            </section>
            <section aria-label="Activity" className="grid">
              <StatTile label="Daily active users" value={formatCount(o.dau)} />
              <StatTile label="Monthly active users" value={formatCount(o.mau)} />
              <StatTile label="Sessions online" value={formatCount(o.gateway?.sessions_online)} hint={`${formatCount(o.gateway?.guild_processes)} guild processes`} />
              <StatTile label="Voice minutes, 24 h" value={formatMinutes(o.voice_minutes_24h)} hint={`${formatCount(o.voice_users)} in voice now across ${formatCount(o.voice_nodes)} nodes`} />
            </section>
            <section aria-label="Messages in the last 24 hours" className="panel">
              <div className="panel-head">
                <div>
                  <h2>Messages, 24 h</h2>
                  <p>Counts only. Message content never reaches the admin portal.</p>
                </div>
              </div>
              <div className="grid">
                <StatTile label="Total" value={formatCount(o.messages_24h?.total)} />
                <StatTile label="Standard" value={formatCount(o.messages_24h?.standard)} />
                <StatTile label="Ephemeral (E2EE)" value={formatCount(o.messages_24h?.ephemeral)} />
                <StatTile label="P2P" value={formatCount(o.messages_24h?.p2p)} />
              </div>
            </section>
            <p className="muted">Updated {formatTimestamp(o.generated_at)}</p>
          </>
        )}
      </Loaded>
    </div>
  );
}
