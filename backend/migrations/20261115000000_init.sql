-- Baseline migration. Domain tables arrive with M1 (auth & household).
CREATE TABLE schema_info (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) STRICT;

INSERT INTO schema_info (key, value) VALUES ('created_by', 'tessera');
