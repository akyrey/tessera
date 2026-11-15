-- M1: users, passkeys, households, invites and sessions.
-- Timestamps are Unix seconds (INTEGER). IDs are UUIDs stored as 16-byte BLOBs.

DROP TABLE schema_info;

CREATE TABLE users (
    id           BLOB PRIMARY KEY NOT NULL,  -- also the WebAuthn user handle
    display_name TEXT NOT NULL CHECK (length(display_name) BETWEEN 1 AND 64),
    is_admin     INTEGER NOT NULL DEFAULT 0 CHECK (is_admin IN (0, 1)),
    created_at   INTEGER NOT NULL
) STRICT;

CREATE TABLE households (
    id         BLOB PRIMARY KEY NOT NULL,
    name       TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 64),
    created_at INTEGER NOT NULL
) STRICT;

-- A user belongs to exactly one household (UNIQUE user_id).
CREATE TABLE household_members (
    household_id BLOB NOT NULL REFERENCES households (id) ON DELETE CASCADE,
    user_id      BLOB NOT NULL UNIQUE REFERENCES users (id) ON DELETE CASCADE,
    role         TEXT NOT NULL CHECK (role IN ('owner', 'member')),
    joined_at    INTEGER NOT NULL,
    PRIMARY KEY (household_id, user_id)
) STRICT;

CREATE TABLE passkeys (
    id            BLOB PRIMARY KEY NOT NULL,
    user_id       BLOB NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    credential_id BLOB NOT NULL UNIQUE,
    passkey       TEXT NOT NULL,  -- webauthn-rs Passkey, JSON
    name          TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 64),
    created_at    INTEGER NOT NULL,
    last_used_at  INTEGER
) STRICT;
CREATE INDEX passkeys_user_id ON passkeys (user_id);

-- bootstrap: first admin + new household (only while no users exist)
-- household: join an existing household as a member
-- recovery:  add a passkey to an existing user (admin CLI)
CREATE TABLE invites (
    id           BLOB PRIMARY KEY NOT NULL,
    token_hash   BLOB NOT NULL UNIQUE,  -- SHA-256 of the secret in the link
    kind         TEXT NOT NULL CHECK (kind IN ('bootstrap', 'household', 'recovery')),
    household_id BLOB REFERENCES households (id) ON DELETE CASCADE,
    user_id      BLOB REFERENCES users (id) ON DELETE CASCADE,
    created_by   BLOB REFERENCES users (id) ON DELETE SET NULL,
    created_at   INTEGER NOT NULL,
    expires_at   INTEGER NOT NULL,
    used_at      INTEGER,
    CHECK ((kind = 'household') = (household_id IS NOT NULL)),
    CHECK ((kind = 'recovery') = (user_id IS NOT NULL))
) STRICT;

CREATE TABLE sessions (
    token_hash BLOB PRIMARY KEY NOT NULL,  -- SHA-256 of the cookie value
    user_id    BLOB NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
) STRICT;
CREATE INDEX sessions_user_id ON sessions (user_id);
CREATE INDEX sessions_expires_at ON sessions (expires_at);
