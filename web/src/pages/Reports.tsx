import { type FormEvent, useEffect, useState } from "react";
import { api } from "../api/client";
import type { ReportResolution, ReportStatus, ReportSummary } from "../api/types";
import { Dialog, ErrorBanner, PageHead, StatusBadge } from "../components/common";
import { formatTimestamp } from "../format";

const asError = (error: unknown) => (error instanceof Error ? error : new Error(String(error)));

const STATUSES: { value: ReportStatus; label: string }[] = [
  { value: "open", label: "Open" },
  { value: "resolved", label: "Resolved" },
  { value: "dismissed", label: "Dismissed" },
];

const CATEGORY_LABELS: Record<string, string> = {
  spam: "Spam",
  harassment: "Harassment",
  hate_speech: "Hate speech",
  violence: "Violence",
  sexual_content: "Sexual content",
  self_harm: "Self-harm",
  illegal: "Illegal content",
  impersonation: "Impersonation",
  other: "Other",
};

export const categoryLabel = (category: string) => CATEGORY_LABELS[category] ?? category;

export function ReportsPage() {
  const [status, setStatus] = useState<ReportStatus>("open");
  const [reports, setReports] = useState<ReportSummary[]>();
  const [next, setNext] = useState<string | null>(null);
  const [error, setError] = useState<Error>();
  const [selected, setSelected] = useState<ReportSummary>();
  const [notice, setNotice] = useState<string>();

  const load = async (after?: string) => {
    try {
      const page = await api.reports(status, after);
      setReports((current) => (after ? [...(current ?? []), ...page.reports] : page.reports));
      setNext(page.next_after);
      setError(undefined);
    } catch (reason) {
      setError(asError(reason));
    }
  };

  useEffect(() => {
    setReports(undefined);
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status]);

  return (
    <div className="stack">
      <PageHead
        title="Reports"
        subtitle="Messages, users and public server listings reported from the apps. Resolving and dismissing are written to the audit log; reporters are never shown to the reported user."
      >
        <div className="segmented" role="group" aria-label="Report status">
          {STATUSES.map((s) => (
            <button key={s.value} type="button" aria-pressed={status === s.value} onClick={() => setStatus(s.value)}>
              {s.label}
            </button>
          ))}
        </div>
      </PageHead>
      {notice ? (
        <p role="status" className="muted">
          {notice}
        </p>
      ) : null}
      {error ? <ErrorBanner error={error} onRetry={() => void load()} /> : null}
      <section className="panel table-wrap" aria-label="Reports">
        {reports && reports.length === 0 ? (
          <div className="empty">No {status} reports.</div>
        ) : reports ? (
          <table>
            <thead>
              <tr>
                <th>Filed</th>
                <th>Category</th>
                <th>Reported</th>
                <th>Type</th>
                <th>Evidence</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {reports.map((r) => (
                <tr key={r.id}>
                  <td>{formatTimestamp(r.created_at)}</td>
                  <td>{categoryLabel(r.category)}</td>
                  <td>
                    {r.kind === "guild" ? <>Guild <span className="mono">{r.guild_id}</span></> : <>@{r.reported.username ?? "unknown"}<div className="muted mono">{r.reported.id}</div></>}
                  </td>
                  <td>{r.kind === "message" ? "Message" : r.kind === "guild" ? "Guild listing" : "User"}</td>
                  <td>
                    <EvidenceBadge report={r} />
                  </td>
                  <td>
                    <button type="button" className="btn small" aria-label={`Review report ${r.id}`} onClick={() => setSelected(r)}>
                      Review
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : null}
        {next ? (
          <button type="button" className="btn" onClick={() => void load(next)}>
            Load more
          </button>
        ) : null}
      </section>
      {selected ? (
        <ReviewDialog
          report={selected}
          onClose={() => setSelected(undefined)}
          onDone={(resolution) => {
            setNotice(`Report ${selected.id} ${resolution}.`);
            setSelected(undefined);
            void load();
          }}
        />
      ) : null}
    </div>
  );
}

function EvidenceBadge({ report }: { report: ReportSummary }) {
  if (report.kind === "guild") return <StatusBadge tone="neutral" label="Public listing" />;
  if (report.kind === "user") return <span className="muted">Profile</span>;
  if (report.content_source === "server") return <StatusBadge tone="good" label="Copied by server" />;
  if (report.content_source === "reporter") return <StatusBadge tone="warning" label="From reporter" />;
  return <StatusBadge tone="neutral" label="None" />;
}

function ReviewDialog({
  report,
  onClose,
  onDone,
}: {
  report: ReportSummary;
  onClose: () => void;
  onDone: (resolution: ReportResolution) => void;
}) {
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<Error>();
  const open = report.status === "open";

  const close = async (resolution: ReportResolution) => {
    setBusy(true);
    try {
      await api.resolveReport(report.id, resolution, note.trim() || undefined);
      onDone(resolution);
    } catch (reason) {
      setError(asError(reason));
      setBusy(false);
    }
  };

  const submit = (event: FormEvent) => event.preventDefault();

  return (
    <Dialog title={`Report ${report.id}`} onClose={onClose}>
      <form className="stack" style={{ gap: 12 }} onSubmit={submit}>
        <dl className="kv">
          <dt>Category</dt>
          <dd>{categoryLabel(report.category)}</dd>
          <dt>Reporter</dt>
          <dd>
            @{report.reporter.username ?? "unknown"} <span className="mono muted">{report.reporter.id}</span>
          </dd>
          {report.kind === "guild" && <><dt>Guild</dt><dd className="mono">{report.guild_id}</dd></>}
          <dt>{report.kind === "guild" ? "Owner at filing" : "Reported"}</dt>
          <dd>
            @{report.reported.username ?? "unknown"} <span className="mono muted">{report.reported.id}</span>
          </dd>
          {report.channel_id ? (
            <>
              <dt>Where</dt>
              <dd className="mono">
                channel {report.channel_id}
                {report.guild_id ? `, guild ${report.guild_id}` : ", direct message"}
                {report.message_id ? `, message ${report.message_id}` : ""}
                {report.delivery_mode ? ` (${report.delivery_mode})` : ""}
              </dd>
            </>
          ) : null}
          {report.note ? (
            <>
              <dt>Reporter note</dt>
              <dd>{report.note}</dd>
            </>
          ) : null}
        </dl>
        <Evidence report={report} />
        {open ? (
          <div className="field">
            <label htmlFor="report-note">Note (recorded in the audit log, optional)</label>
            <textarea id="report-note" rows={2} maxLength={500} value={note} onChange={(e) => setNote(e.target.value)} />
          </div>
        ) : (
          <p className="muted">
            {report.status === "resolved" ? "Resolved" : "Dismissed"} by {report.resolved_by ?? "unknown"}
            {report.resolved_at ? ` on ${formatTimestamp(report.resolved_at)}` : ""}
            {report.resolution_note ? `: ${report.resolution_note}` : "."}
          </p>
        )}
        {error ? <ErrorBanner error={error} /> : null}
        <div className="form-row" style={{ justifyContent: "flex-end" }}>
          <button type="button" className="btn" onClick={onClose}>
            {open ? "Cancel" : "Close"}
          </button>
          {open ? (
            <>
              <button type="button" className="btn" disabled={busy} onClick={() => void close("dismissed")}>
                Dismiss report
              </button>
              <button type="button" className="btn primary" disabled={busy} onClick={() => void close("resolved")}>
                Mark resolved
              </button>
            </>
          ) : null}
        </div>
      </form>
    </Dialog>
  );
}

function Evidence({ report }: { report: ReportSummary }) {
  if (report.kind === "guild") {
    return <div className="field">
      <span className="label">Public listing, copied from the server when the report was filed</span>
      <blockquote className="evidence">{report.content}</blockquote>
      <p className="muted">Private channels, members and messages are not included. Resolving this report does not remove the listing.</p>
    </div>;
  }
  if (report.kind === "user") {
    return <p className="muted">User report: no message attached. Look the account up under Trust &amp; Safety to suspend it.</p>;
  }
  if (report.content_source === "server") {
    return (
      <div className="field">
        <span className="label">Message content, copied from the server when the report was filed</span>
        <blockquote className="evidence">{report.content || "(no text, the message may only have had attachments)"}</blockquote>
      </div>
    );
  }
  if (report.content_source === "reporter") {
    return (
      <div className="field">
        <span className="label">Text submitted by the reporter. Unverified: this chat is {report.delivery_mode}, so the server holds no copy.</span>
        <blockquote className="evidence">{report.content}</blockquote>
      </div>
    );
  }
  return (
    <p className="muted">
      No content attached. This chat is {report.delivery_mode ?? "not stored"}, so the server holds no copy and the reporter submitted none.
    </p>
  );
}
