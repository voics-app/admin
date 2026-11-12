import { type FormEvent, useState } from "react";
import { api } from "../api/client";
import type { NotificationDraft, PushReport, UserSummary } from "../api/types";
import { BRAND } from "../brand";
import { Dialog, ErrorBanner, PageHead } from "../components/common";
import { formatCount } from "../format";

const MAX_TITLE = 64;
const MAX_BODY = 240;

const asError = (error: unknown) => (error instanceof Error ? error : new Error(String(error)));

type Audience = "user" | "everyone";

export function SendNotificationPage() {
  const [audience, setAudience] = useState<Audience>("user");
  const [user, setUser] = useState<UserSummary>();
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [review, setReview] = useState<{ draft: NotificationDraft; report: PushReport }>();
  const [error, setError] = useState<Error>();
  const [notice, setNotice] = useState<string>();
  const [busy, setBusy] = useState(false);

  const ready = title.trim() !== "" && body.trim() !== "" && (audience === "everyone" || user !== undefined);

  const draft = (): NotificationDraft => ({
    target: audience === "everyone" ? "all" : { user_id: user!.id },
    title: title.trim(),
    body: body.trim(),
  });

  // The preview asks the notifier how many devices would receive it, so the confirmation is exact.
  const startReview = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setNotice(undefined);
    try {
      const next = draft();
      setReview({ draft: next, report: await api.previewNotification(next) });
      setError(undefined);
    } catch (reason) {
      setError(asError(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="stack">
      <PageHead
        title="Send notification"
        subtitle="Send a push notification by hand to one user or to everyone. Every send is written to the audit log."
      />
      {notice ? (
        <div className="banner" role="status">
          {notice}
        </div>
      ) : null}
      <section className="panel" aria-label="Notification">
        <form className="stack" style={{ gap: 16 }} onSubmit={(event) => void startReview(event)}>
          <div className="field">
            <span className="label" id="audience-label">
              Send to
            </span>
            <div className="segmented" role="group" aria-labelledby="audience-label">
              <button type="button" aria-pressed={audience === "user"} onClick={() => setAudience("user")}>
                One user
              </button>
              <button type="button" aria-pressed={audience === "everyone"} onClick={() => setAudience("everyone")}>
                Everyone
              </button>
            </div>
          </div>
          {audience === "user" ? <UserPicker user={user} onPick={setUser} /> : null}
          <div className="field">
            <label htmlFor="push-title">Title</label>
            <input id="push-title" maxLength={MAX_TITLE} value={title} onChange={(e) => setTitle(e.target.value)} />
            <span className="muted">
              {title.length}/{MAX_TITLE}
            </span>
          </div>
          <div className="field">
            <label htmlFor="push-body">Message</label>
            <textarea id="push-body" rows={4} maxLength={MAX_BODY} value={body} onChange={(e) => setBody(e.target.value)} />
            <span className="muted">
              {body.length}/{MAX_BODY}
            </span>
          </div>
          <Preview title={title} body={body} />
          {error ? <ErrorBanner error={error} /> : null}
          <div className="form-row">
            <button type="submit" className="btn primary" disabled={!ready || busy}>
              Review and send
            </button>
          </div>
        </form>
      </section>
      {review ? (
        <ConfirmDialog
          review={review}
          userLabel={user ? `@${user.username}` : ""}
          onClose={() => setReview(undefined)}
          onSent={(report) => {
            setReview(undefined);
            setTitle("");
            setBody("");
            setNotice(
              `Sent to ${formatCount(report.queued)} of ${formatCount(report.devices)} devices` +
                (report.dropped > 0 ? `; ${formatCount(report.dropped)} could not be queued.` : "."),
            );
          }}
        />
      ) : null}
    </div>
  );
}

function Preview({ title, body }: { title: string; body: string }) {
  return (
    <div className="field">
      <span className="label">Preview</span>
      <div className="push-preview" aria-label="Notification preview">
        <div className="push-app">{BRAND.name}</div>
        <div className="push-title">{title.trim() || "Title"}</div>
        <div className="push-body">{body.trim() || "Message"}</div>
      </div>
    </div>
  );
}

function UserPicker({ user, onPick }: { user?: UserSummary; onPick: (user?: UserSummary) => void }) {
  const [text, setText] = useState("");
  const [results, setResults] = useState<UserSummary[]>();
  const [error, setError] = useState<Error>();

  const search = async () => {
    try {
      setResults((await api.searchUsers(text.trim())).users.filter((u) => !u.bot));
      setError(undefined);
    } catch (reason) {
      setError(asError(reason));
    }
  };

  if (user) {
    return (
      <div className="field">
        <span className="label">Recipient</span>
        <div className="form-row">
          <span>
            {user.global_name ?? user.username} <span className="muted">@{user.username}</span> <span className="mono">{user.id}</span>
          </span>
          <button type="button" className="btn small" onClick={() => onPick(undefined)}>
            Change recipient
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="field">
      <label htmlFor="push-user-query">Find the recipient</label>
      <div className="form-row">
        <input
          id="push-user-query"
          type="search"
          placeholder="Username, email or ID"
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void search();
            }
          }}
        />
        <button type="button" className="btn" onClick={() => void search()}>
          Search recipient
        </button>
      </div>
      {error ? <ErrorBanner error={error} /> : null}
      {results ? (
        results.length === 0 ? (
          <div className="empty">No users match.</div>
        ) : (
          <ul className="pick-list" aria-label="Matching users">
            {results.map((u) => (
              <li key={u.id}>
                <span>
                  {u.global_name ?? u.username} <span className="muted">@{u.username}</span> <span className="mono">{u.id}</span>
                </span>
                <button type="button" className="btn small" aria-label={`Choose ${u.username}`} onClick={() => onPick(u)}>
                  Choose
                </button>
              </li>
            ))}
          </ul>
        )
      ) : null}
    </div>
  );
}

function ConfirmDialog({
  review,
  userLabel,
  onClose,
  onSent,
}: {
  review: { draft: NotificationDraft; report: PushReport };
  userLabel: string;
  onClose: () => void;
  onSent: (report: PushReport) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<Error>();
  const everyone = review.draft.target === "all";
  const devices = review.report.devices;

  const send = async () => {
    setBusy(true);
    try {
      onSent(await api.sendNotification(review.draft));
    } catch (reason) {
      setError(asError(reason));
      setBusy(false);
    }
  };

  return (
    <Dialog title={everyone ? "Send to everyone?" : `Send to ${userLabel}?`} onClose={onClose}>
      <div className="stack" style={{ gap: 12 }}>
        <Preview title={review.draft.title} body={review.draft.body} />
        {devices === 0 ? (
          <div className="banner warning" role="status">
            {everyone ? "No device is registered for push, so nobody would receive it." : "This user has no device registered for push, so nothing would be delivered."}
          </div>
        ) : everyone ? (
          <p role="status">
            This will be sent to <strong>{formatCount(devices)} devices</strong> right now. It cannot be recalled.
            {review.report.truncated ? " Only the first part of the audience is included (the limit for one send)." : ""}
          </p>
        ) : (
          <p role="status">
            It will be sent to <strong>{formatCount(devices)}</strong> {devices === 1 ? "device" : "devices"} of this user.
          </p>
        )}
        {error ? <ErrorBanner error={error} /> : null}
        <div className="form-row" style={{ justifyContent: "flex-end" }}>
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="button" className="btn danger" disabled={busy || devices === 0} onClick={() => void send()}>
            {everyone ? `Send to ${formatCount(devices)} devices` : "Send notification"}
          </button>
        </div>
      </div>
    </Dialog>
  );
}
