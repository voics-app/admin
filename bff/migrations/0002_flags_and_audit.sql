CREATE TABLE feature_flags (
    key             TEXT        PRIMARY KEY,
    description     TEXT        NOT NULL DEFAULT '',
    enabled         BOOLEAN     NOT NULL DEFAULT FALSE,
    rollout_percent SMALLINT    NOT NULL DEFAULT 0 CHECK (rollout_percent BETWEEN 0 AND 100),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by      TEXT        NOT NULL
);

-- Append-only record of every privileged admin action.
CREATE TABLE admin_audit (
    id          BIGSERIAL   PRIMARY KEY,
    at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor_email TEXT        NOT NULL,
    action      TEXT        NOT NULL,
    target      TEXT        NOT NULL,
    details     JSONB       NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX admin_audit_at_idx ON admin_audit (at DESC);
