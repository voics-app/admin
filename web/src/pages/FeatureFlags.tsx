import { type FormEvent, useState } from "react";
import { api } from "../api/client";
import type { Flag } from "../api/types";
import { useAsync } from "../api/useAsync";
import { Dialog, ErrorBanner, Loaded, PageHead } from "../components/common";
import { formatTimestamp } from "../format";

const asError = (error: unknown) => (error instanceof Error ? error : new Error(String(error)));

type Editing = { mode: "create" } | { mode: "edit"; flag: Flag };

export function FeatureFlagsPage() {
  const state = useAsync(api.flags, []);
  const [editing, setEditing] = useState<Editing>();
  const [removing, setRemoving] = useState<Flag>();
  const [failure, setFailure] = useState<Error>();

  const remove = async () => {
    if (!removing) return;
    try {
      await api.deleteFlag(removing.key);
      setFailure(undefined);
      state.reload();
    } catch (error) {
      setFailure(asError(error));
    }
    setRemoving(undefined);
  };

  return (
    <div className="stack">
      <PageHead title="Feature flags" subtitle="Percentage rollouts read by the api. Every change is audited.">
        <button type="button" className="btn primary" onClick={() => setEditing({ mode: "create" })}>
          New flag
        </button>
      </PageHead>
      {failure ? <ErrorBanner error={failure} /> : null}
      <Loaded state={state}>
        {({ flags }) =>
          flags.length === 0 ? (
            <div className="panel empty">No flags yet.</div>
          ) : (
            <section className="panel table-wrap" aria-label="Flags">
              <table>
                <thead>
                  <tr>
                    <th>Key</th>
                    <th>Description</th>
                    <th>State</th>
                    <th className="num">Rollout</th>
                    <th>Updated</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {flags.map((flag) => (
                    <tr key={flag.key}>
                      <td className="mono">{flag.key}</td>
                      <td style={{ whiteSpace: "normal" }}>{flag.description || <span className="muted">n/a</span>}</td>
                      <td>{flag.enabled ? "On" : "Off"}</td>
                      <td className="num">{flag.rollout_percent}%</td>
                      <td>
                        {formatTimestamp(flag.updated_at)}
                        <div className="muted">{flag.updated_by}</div>
                      </td>
                      <td>
                        <div className="form-row">
                          <button type="button" className="btn small" aria-label={`Edit ${flag.key}`} onClick={() => setEditing({ mode: "edit", flag })}>
                            Edit
                          </button>
                          <button type="button" className="btn small danger" aria-label={`Delete ${flag.key}`} onClick={() => setRemoving(flag)}>
                            Delete
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          )
        }
      </Loaded>
      {editing ? (
        <FlagDialog
          editing={editing}
          onClose={() => setEditing(undefined)}
          onSaved={() => {
            setEditing(undefined);
            state.reload();
          }}
        />
      ) : null}
      {removing ? (
        <Dialog title={`Delete ${removing.key}?`} onClose={() => setRemoving(undefined)}>
          <p className="muted" style={{ margin: 0 }}>
            Clients that read this flag fall back to their default.
          </p>
          <div className="form-row" style={{ justifyContent: "flex-end" }}>
            <button type="button" className="btn" onClick={() => setRemoving(undefined)}>
              Cancel
            </button>
            <button type="button" className="btn danger" onClick={() => void remove()}>
              Delete flag
            </button>
          </div>
        </Dialog>
      ) : null}
    </div>
  );
}

function FlagDialog({ editing, onClose, onSaved }: { editing: Editing; onClose: () => void; onSaved: () => void }) {
  const existing = editing.mode === "edit" ? editing.flag : undefined;
  const [key, setKey] = useState(existing?.key ?? "");
  const [description, setDescription] = useState(existing?.description ?? "");
  const [enabled, setEnabled] = useState(existing?.enabled ?? false);
  const [rollout, setRollout] = useState(existing?.rollout_percent ?? 100);
  const [error, setError] = useState<Error>();
  const [busy, setBusy] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    try {
      await api.putFlag(key.trim(), { description: description.trim(), enabled, rollout_percent: rollout });
      onSaved();
    } catch (reason) {
      setError(asError(reason));
      setBusy(false);
    }
  };

  return (
    <Dialog title={existing ? `Edit ${existing.key}` : "New flag"} onClose={onClose}>
      <form className="stack" style={{ gap: 12 }} onSubmit={(e) => void submit(e)}>
        <div className="field">
          <label htmlFor="flag-key">Key</label>
          <input id="flag-key" type="text" required readOnly={existing !== undefined} value={key} onChange={(e) => setKey(e.target.value)} placeholder="voice.video_grid" />
        </div>
        <div className="field">
          <label htmlFor="flag-description">Description</label>
          <input id="flag-description" type="text" value={description} onChange={(e) => setDescription(e.target.value)} />
        </div>
        <div className="field">
          <label htmlFor="flag-enabled">
            <input id="flag-enabled" type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} /> Enabled
          </label>
        </div>
        <div className="field">
          <label htmlFor="flag-rollout">Rollout percentage</label>
          <input id="flag-rollout" type="number" min={0} max={100} step={1} value={rollout} onChange={(e) => setRollout(Number(e.target.value))} />
        </div>
        {error ? <ErrorBanner error={error} /> : null}
        <div className="form-row" style={{ justifyContent: "flex-end" }}>
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="submit" className="btn primary" disabled={busy || key.trim() === ""}>
            Save flag
          </button>
        </div>
      </form>
    </Dialog>
  );
}
