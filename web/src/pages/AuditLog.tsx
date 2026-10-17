import { useEffect, useState } from "react";
import { api } from "../api/client";
import type { AuditEntry } from "../api/types";
import { ErrorBanner, PageHead } from "../components/common";
import { formatTimestamp } from "../format";

export function AuditLogPage() {
  const [entries, setEntries] = useState<AuditEntry[]>([]);
  const [next, setNext] = useState<number | null>(null);
  const [error, setError] = useState<Error>();
  const [loading, setLoading] = useState(true);

  const load = async (before?: number) => {
    setLoading(true);
    try {
      const page = await api.audit(before);
      setEntries((current) => (before === undefined ? page.entries : [...current, ...page.entries]));
      setNext(page.next_before);
      setError(undefined);
    } catch (reason) {
      setError(reason instanceof Error ? reason : new Error(String(reason)));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
  }, []);

  return (
    <div className="stack">
      <PageHead title="Audit log" subtitle="Every administrative action, newest first." />
      {error ? <ErrorBanner error={error} onRetry={() => void load()} /> : null}
      <section className="panel table-wrap" aria-label="Audit entries">
        {entries.length === 0 && !loading && !error ? (
          <div className="empty">No administrative actions recorded yet.</div>
        ) : (
          <table>
            <thead>
              <tr>
                <th>When</th>
                <th>Admin</th>
                <th>Action</th>
                <th>Target</th>
                <th>Details</th>
              </tr>
            </thead>
            <tbody>
              {entries.map((entry) => (
                <tr key={entry.id}>
                  <td>{formatTimestamp(entry.at)}</td>
                  <td>{entry.actor_email}</td>
                  <td className="mono">{entry.action}</td>
                  <td className="mono">{entry.target}</td>
                  <td>
                    <code>{entry.details == null ? "" : JSON.stringify(entry.details)}</code>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {next !== null ? (
          <div style={{ paddingTop: 12 }}>
            <button type="button" className="btn" disabled={loading} onClick={() => void load(next)}>
              Load older entries
            </button>
          </div>
        ) : null}
      </section>
    </div>
  );
}
