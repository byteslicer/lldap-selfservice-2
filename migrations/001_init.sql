CREATE TABLE IF NOT EXISTS invites (
    id UUID PRIMARY KEY,
    token_hash BYTEA NOT NULL UNIQUE,
    created_by_uid TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    used_at TIMESTAMPTZ,
    lldap_user_id TEXT,
    label TEXT
);

CREATE INDEX IF NOT EXISTS idx_invites_created_at ON invites (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_invites_used_at ON invites (used_at);

CREATE TABLE IF NOT EXISTS sessions (
    id UUID PRIMARY KEY,
    uid TEXT NOT NULL,
    can_invite BOOLEAN NOT NULL,
    can_reset_pwd BOOLEAN NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    csrf_token TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions (expires_at);
