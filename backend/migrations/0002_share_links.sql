-- migrations/0002_share_links.sql -------------------------------------------
-- F4 public read-only share links. The code is a URL-borne bearer credential:
-- stored ONLY as sha256 (mirroring owners.secret_hash), surfaced in plaintext
-- exactly once in the 201 response, and addressed thereafter by the non-secret
-- integer id so no code ever lands in an owner-route URL (and therefore never
-- in nginx's access log for `location /api/`).

CREATE TABLE share_links (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,  -- non-secret handle: list + revoke
    code_hash    TEXT NOT NULL UNIQUE,               -- sha256(code) hex, 64 chars; the lookup key
    token        TEXT NOT NULL,                      -- owning endpoint
    label        TEXT,                               -- optional operator note, <= 80 chars, NULL when blank
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    revoked_at   TEXT,                               -- non-null => dead; never un-set, never deleted
    last_used_at TEXT,                               -- best-effort, coalesced to >= 60s granularity
    FOREIGN KEY (token) REFERENCES endpoints(token) ON DELETE CASCADE
);

-- Public resolver: exact-match on the UNIQUE code_hash index (one B-tree probe).
-- Owner list + the active-count cap check: covered by this composite index.
CREATE INDEX idx_share_token_active ON share_links(token, revoked_at, created_at DESC);
