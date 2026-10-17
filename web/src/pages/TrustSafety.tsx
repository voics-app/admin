import { type FormEvent, useState } from "react";
import { api } from "../api/client";
import type { GuildSummary, UserSummary } from "../api/types";
import { Dialog, ErrorBanner, PageHead, StatusBadge } from "../components/common";
import { formatCount } from "../format";

const asError = (error: unknown) => (error instanceof Error ? error : new Error(String(error)));

export function TrustSafetyPage() {
  return (
    <div className="stack">
      <PageHead title="Trust & Safety" subtitle="Look up accounts and guilds. Suspensions are written to the audit log." />
      <UserSearch />
      <GuildSearch />
    </div>
  );
}

function UserSearch() {
  const [text, setText] = useState("");
  const [users, setUsers] = useState<UserSummary[]>();
  const [next, setNext] = useState<string | null>(null);
  const [error, setError] = useState<Error>();
  const [target, setTarget] = useState<UserSummary>();
  const [notice, setNotice] = useState<string>();

  const load = async (after?: string) => {
    try {
      const page = await api.searchUsers(text.trim(), after);
      setUsers((current) => (after ? [...(current ?? []), ...page.users] : page.users));
      setNext(page.next_after);
      setError(undefined);
    } catch (reason) {
      setError(asError(reason));
    }
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    void load();
  };

  return (
    <section className="panel" aria-label="Users">
      <div className="panel-head">
        <h2>Users</h2>
      </div>
      <form className="form-row" onSubmit={submit} role="search">
        <label className="sr-only" htmlFor="user-query">
          Search users
        </label>
        <input id="user-query" type="search" placeholder="Username, email or ID" value={text} onChange={(e) => setText(e.target.value)} />
        <button type="submit" className="btn primary">
          Search users
        </button>
      </form>
      {notice ? (
        <p role="status" className="muted">
          {notice}
        </p>
      ) : null}
      {error ? <ErrorBanner error={error} /> : null}
      {users ? (
        users.length === 0 ? (
          <div className="empty">No users match.</div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>User</th>
                  <th>ID</th>
                  <th>Email</th>
                  <th>Status</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {users.map((u) => (
                  <tr key={u.id}>
                    <td>
                      {u.global_name ?? u.username} {u.bot ? <span className="status">Bot</span> : null}
                      <div className="muted">@{u.username}</div>
                    </td>
                    <td className="mono">{u.id}</td>
                    <td>{u.email ?? "n/a"}</td>
                    <td>{u.suspended_until ? <StatusBadge tone="serious" label={`Suspended until ${u.suspended_until}`} /> : <StatusBadge tone="good" label="Active" />}</td>
                    <td>
                      <button type="button" className="btn small danger" aria-label={`Suspend ${u.username}`} onClick={() => setTarget(u)}>
                        Suspend
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            {next ? (
              <button type="button" className="btn" onClick={() => void load(next)}>
                Load more
              </button>
            ) : null}
          </div>
        )
      ) : null}
      {target ? (
        <SuspendDialog
          user={target}
          onClose={() => setTarget(undefined)}
          onDone={() => {
            setNotice(`Suspended @${target.username}.`);
            setTarget(undefined);
            void load();
          }}
        />
      ) : null}
    </section>
  );
}

function SuspendDialog({ user, onClose, onDone }: { user: UserSummary; onClose: () => void; onDone: () => void }) {
  const [reason, setReason] = useState("");
  const [until, setUntil] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<Error>();

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    try {
      await api.suspendUser(user.id, reason.trim(), until ? new Date(until).toISOString() : undefined);
      onDone();
    } catch (reason) {
      setError(asError(reason));
      setBusy(false);
    }
  };

  return (
    <Dialog title={`Suspend @${user.username}`} onClose={onClose}>
      <form className="stack" style={{ gap: 12 }} onSubmit={submit}>
        <div className="field">
          <label htmlFor="suspend-reason">Reason (recorded in the audit log)</label>
          <textarea id="suspend-reason" required rows={3} value={reason} onChange={(e) => setReason(e.target.value)} />
        </div>
        <div className="field">
          <label htmlFor="suspend-until">Until (leave empty for indefinite)</label>
          <input id="suspend-until" type="datetime-local" value={until} onChange={(e) => setUntil(e.target.value)} />
        </div>
        {error ? <ErrorBanner error={error} /> : null}
        <div className="form-row" style={{ justifyContent: "flex-end" }}>
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="submit" className="btn danger" disabled={busy || reason.trim() === ""}>
            Suspend user
          </button>
        </div>
      </form>
    </Dialog>
  );
}

function GuildSearch() {
  const [text, setText] = useState("");
  const [guilds, setGuilds] = useState<GuildSummary[]>();
  const [error, setError] = useState<Error>();

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    try {
      setGuilds((await api.searchGuilds(text.trim())).guilds);
      setError(undefined);
    } catch (reason) {
      setError(asError(reason));
    }
  };

  return (
    <section className="panel" aria-label="Guilds">
      <div className="panel-head">
        <h2>Guilds</h2>
      </div>
      <form className="form-row" onSubmit={(e) => void submit(e)} role="search">
        <label className="sr-only" htmlFor="guild-query">
          Search guilds
        </label>
        <input id="guild-query" type="search" placeholder="Guild name or ID" value={text} onChange={(e) => setText(e.target.value)} />
        <button type="submit" className="btn primary">
          Search guilds
        </button>
      </form>
      {error ? <ErrorBanner error={error} /> : null}
      {guilds ? (
        guilds.length === 0 ? (
          <div className="empty">No guilds match.</div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Guild</th>
                  <th>ID</th>
                  <th>Owner</th>
                  <th className="num">Members</th>
                  <th>Voice region</th>
                </tr>
              </thead>
              <tbody>
                {guilds.map((g) => (
                  <tr key={g.id}>
                    <td>{g.name}</td>
                    <td className="mono">{g.id}</td>
                    <td className="mono">{g.owner_id ?? "n/a"}</td>
                    <td className="num">{formatCount(g.member_count)}</td>
                    <td>{g.voice_region ?? "automatic"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )
      ) : null}
    </section>
  );
}
