-- M5: store brands (Wikidata), their locations (OpenStreetMap) and custom places.

CREATE TABLE brands (
    id             BLOB PRIMARY KEY NOT NULL,
    wikidata_id    TEXT NOT NULL UNIQUE CHECK (wikidata_id GLOB 'Q[1-9]*' AND length(wikidata_id) <= 13),
    name           TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 128),
    created_at     INTEGER NOT NULL,
    -- Last successful Overpass fetch; NULL until the first one.
    last_synced_at INTEGER,
    -- Last failed attempt, so the refresh job backs off.
    last_error_at  INTEGER
) STRICT;

CREATE TABLE store_locations (
    brand_id BLOB NOT NULL REFERENCES brands (id) ON DELETE CASCADE,
    osm_type TEXT NOT NULL CHECK (osm_type IN ('node', 'way', 'relation')),
    osm_id   INTEGER NOT NULL,
    lat      REAL NOT NULL CHECK (lat BETWEEN -90 AND 90),
    lon      REAL NOT NULL CHECK (lon BETWEEN -180 AND 180),
    name     TEXT,
    PRIMARY KEY (brand_id, osm_type, osm_id)
) STRICT;

ALTER TABLE cards ADD COLUMN brand_id BLOB REFERENCES brands (id) ON DELETE SET NULL;
CREATE INDEX cards_brand_id ON cards (brand_id) WHERE brand_id IS NOT NULL;

-- Shops missing from OpenStreetMap, marked by hand. Replaced as a whole on card save.
CREATE TABLE card_places (
    card_id  BLOB NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    label    TEXT NOT NULL CHECK (length(label) BETWEEN 1 AND 64),
    lat      REAL NOT NULL CHECK (lat BETWEEN -90 AND 90),
    lon      REAL NOT NULL CHECK (lon BETWEEN -180 AND 180),
    PRIMARY KEY (card_id, position)
) STRICT;
