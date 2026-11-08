-- migrations/0004_anonymous_owner_key.sql -----------------------------------
-- Anonymous owner identity (hookbox-a73.3): drop `owners.email` and make the
-- owner key's hash unique. An owner is now a random 64-bit hex id plus a
-- secret key carried in an HttpOnly cookie (or as a Bearer for the CLI);
-- the server stores only sha256(key).
--
-- SQLite cannot drop a UNIQUE column in place, so the table is rebuilt:
--   * sqlx-sqlite runs every migration inside a transaction, where
--     `PRAGMA foreign_keys = OFF` is a silent no-op. `defer_foreign_keys`
--     instead postpones the FK check of the implicit `DELETE FROM owners`
--     (done by `DROP TABLE`) to COMMIT, by which point every owner row is
--     back.
--   * The old table is copied to TEMP and dropped, never renamed: with
--     foreign keys on, `ALTER TABLE ... RENAME` rewrites `endpoints`'
--     `REFERENCES owners(owner_id)` to point at the renamed table.
-- Legacy rows keep their owner_id (sha256(email)[:16]) and secret_hash, so
-- an existing owner secret keeps working as a recovery key.

PRAGMA defer_foreign_keys = ON;

CREATE TEMP TABLE owners_0004 AS
    SELECT owner_id, secret_hash, created_at, last_seen FROM main.owners;

DROP TABLE main.owners;

CREATE TABLE main.owners (
    owner_id    TEXT PRIMARY KEY,   -- random 64-bit hex; legacy rows keep sha256(email)[:16]; non-secret
    secret_hash TEXT NOT NULL,      -- sha256(owner key); changes only on explicit rotate
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen   TEXT
);

CREATE UNIQUE INDEX main.idx_owners_secret_hash ON owners(secret_hash);

INSERT INTO main.owners (owner_id, secret_hash, created_at, last_seen)
    SELECT owner_id, secret_hash, created_at, last_seen FROM temp.owners_0004;

DROP TABLE temp.owners_0004;
