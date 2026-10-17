import { useState } from "react";
import { api } from "../api/client";
import type { Region, VoiceNode } from "../api/types";
import { useAsync } from "../api/useAsync";
import { Dialog, ErrorBanner, Loaded, PageHead, StatusBadge } from "../components/common";
import { formatBitrate, formatCount, formatMilliseconds } from "../format";

interface DrainTarget {
  node: VoiceNode;
  drain: boolean;
}

export function VoiceRegionsPage() {
  const state = useAsync(api.voiceRegions, [], 15_000);
  const [pending, setPending] = useState<DrainTarget>();
  const [failure, setFailure] = useState<Error>();
  const [busy, setBusy] = useState(false);

  const confirm = async () => {
    if (!pending) return;
    setBusy(true);
    try {
      await (pending.drain ? api.drainNode(pending.node.node_id) : api.undrainNode(pending.node.node_id));
      setFailure(undefined);
      setPending(undefined);
      state.reload();
    } catch (error) {
      setFailure(error instanceof Error ? error : new Error(String(error)));
      setPending(undefined);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="stack">
      <PageHead title="Voice regions" subtitle="Voice nodes registered in etcd. Draining stops new allocations; live calls finish normally." />
      {failure ? <ErrorBanner error={failure} /> : null}
      <Loaded state={state}>
        {(report) => (
          <>
            {!report.etcd_healthy ? (
              <div className="banner warning" role="status">
                etcd is unreachable. The node list may be stale or empty.
              </div>
            ) : null}
            {report.regions.length === 0 ? <div className="panel empty">No voice nodes are registered.</div> : null}
            {report.regions.map((region) => (
              <RegionPanel
                key={region.region}
                region={region}
                windowMinutes={report.quality_window_minutes}
                onToggle={(node, drain) => setPending({ node, drain })}
              />
            ))}
          </>
        )}
      </Loaded>
      {pending ? (
        <Dialog title={pending.drain ? `Drain ${pending.node.node_id}?` : `Resume ${pending.node.node_id}?`} onClose={() => setPending(undefined)}>
          <p className="muted" style={{ margin: 0 }}>
            {pending.drain
              ? `${formatCount(pending.node.users)} users are connected. New calls will be placed on other nodes; existing calls are not interrupted.`
              : "The node will accept new voice allocations again."}
          </p>
          <div className="form-row" style={{ justifyContent: "flex-end" }}>
            <button type="button" className="btn" onClick={() => setPending(undefined)}>
              Cancel
            </button>
            <button type="button" className={`btn ${pending.drain ? "danger" : "primary"}`} disabled={busy} onClick={confirm}>
              {pending.drain ? "Drain node" : "Resume node"}
            </button>
          </div>
        </Dialog>
      ) : null}
    </div>
  );
}

function RegionPanel({ region, windowMinutes, onToggle }: { region: Region; windowMinutes: number; onToggle: (node: VoiceNode, drain: boolean) => void }) {
  const q = region.quality;
  return (
    <section className="panel" aria-label={`Region ${region.region}`}>
      <div className="panel-head">
        <div>
          <h2>{region.region}</h2>
          <p>
            {region.accepting_nodes} of {region.nodes} nodes accepting, {formatCount(region.users)} users, {formatBitrate(region.egress_bps)} egress, CPU avg {region.avg_cpu_pct.toFixed(0)}% / max {region.max_cpu_pct.toFixed(0)}%
          </p>
        </div>
        {region.accepting_nodes === 0 ? <StatusBadge tone="critical" label="Not accepting" /> : region.draining_nodes > 0 ? <StatusBadge tone="warning" label={`${region.draining_nodes} draining`} /> : <StatusBadge tone="good" label="Accepting" />}
      </div>
      <p className="muted" style={{ marginTop: 0 }}>
        {q
          ? `Call quality, last ${windowMinutes} min (${formatCount(q.samples)} samples): jitter ${formatMilliseconds(q.jitter_ms)}, loss ${q.loss_pct.toFixed(2)}%, RTT ${formatMilliseconds(q.rtt_ms)}, ${q.bitrate_kbps.toFixed(0)} kbit/s`
          : `No call-quality samples in the last ${windowMinutes} min.`}
      </p>
      <div className="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Node</th>
              <th>Address</th>
              <th className="num">Users</th>
              <th className="num">CPU</th>
              <th className="num">Egress</th>
              <th>Version</th>
              <th>State</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {region.node_list.map((node) => (
              <tr key={node.node_id}>
                <td className="mono">{node.node_id}</td>
                <td className="mono">
                  {node.public_ip}:{node.udp_port}
                </td>
                <td className="num">{formatCount(node.users)}</td>
                <td className="num">{node.cpu_pct.toFixed(0)}%</td>
                <td className="num">{formatBitrate(node.egress_bps)}</td>
                <td className="mono">{node.version}</td>
                <td>{node.draining ? <StatusBadge tone="warning" label="Draining" /> : <StatusBadge tone="good" label="Accepting" />}</td>
                <td>
                  <button
                    type="button"
                    className={`btn small ${node.draining ? "" : "danger"}`}
                    aria-label={`${node.draining ? "Resume" : "Drain"} ${node.node_id}`}
                    onClick={() => onToggle(node, !node.draining)}
                  >
                    {node.draining ? "Resume" : "Drain"}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
