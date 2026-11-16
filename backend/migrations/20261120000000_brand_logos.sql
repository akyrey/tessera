-- Brand logos from Wikidata (P154), downloaded once from Wikimedia Commons and
-- served by this server, so phones never contact a third party and work offline.

-- Last logo lookup, successful or not (no logo on Wikidata counts as success).
ALTER TABLE brands ADD COLUMN logo_checked_at INTEGER;
-- Last failed lookup, so the refresh job backs off.
ALTER TABLE brands ADD COLUMN logo_error_at INTEGER;

CREATE TABLE brand_logos (
    -- New id whenever the image changes, so clients can cache it forever.
    id         BLOB PRIMARY KEY NOT NULL,
    brand_id   BLOB NOT NULL UNIQUE REFERENCES brands (id) ON DELETE CASCADE,
    mime       TEXT NOT NULL CHECK (mime IN ('image/jpeg', 'image/png', 'image/webp')),
    data       BLOB NOT NULL CHECK (length(data) BETWEEN 1 AND 262144),
    created_at INTEGER NOT NULL
) STRICT;
