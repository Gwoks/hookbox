-- migrations/0001_init.sql --------------------------------------------------
-- HookBox §5.6 FROZEN schema. Replaces the old SQLite tables AND all four
-- Redis responsibilities (state, crud, pub/sub, rate-limit). Pub/sub and
-- rate-limit are in-process (broadcast + DashMap); state and crud are the two
-- [new] tables below. WAL + foreign_keys + busy_timeout are set at pool open.
-- Timestamps are TEXT RFC3339 (datetime('now')). [existing — app/database.py::_DDL]

CREATE TABLE owners (                                      -- [existing]
    owner_id    TEXT PRIMARY KEY,                          -- sha256(lower(trim(email)))[:16], non-secret
    email       TEXT UNIQUE NOT NULL,
    secret_hash TEXT NOT NULL,                             -- sha256(owner_secret); rotates each /api/session
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen   TEXT
);

CREATE TABLE endpoints (                                   -- [existing] + chaos_mode/gone_at [new]
    token              TEXT PRIMARY KEY,                   -- gen_token: 10-char ambiguity-stripped, case-sensitive
    owner_id           TEXT NOT NULL,
    name               TEXT,
    auto_crud          INTEGER NOT NULL DEFAULT 0,
    target_url         TEXT,
    default_mode       TEXT NOT NULL DEFAULT 'mock_404',   -- 'mock_404' | 'echo'
    latency_ms         INTEGER NOT NULL DEFAULT 0,
    rate_limit_per_min INTEGER NOT NULL DEFAULT 0,
    chaos_pct          INTEGER NOT NULL DEFAULT 0,
    chaos_mode         TEXT NOT NULL DEFAULT 'error',      -- [new] OQ-2: 'error' | 'dropout'
    cors_enabled       INTEGER NOT NULL DEFAULT 1,
    gone_at            TEXT,                                -- [new] OQ-1: tombstone; non-null => 410 endpoint_gone
    created_at         TEXT NOT NULL DEFAULT (datetime('now')),
    last_hit           TEXT,
    request_count      INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (owner_id) REFERENCES owners(owner_id)
);
CREATE INDEX idx_endpoints_owner ON endpoints(owner_id);

CREATE TABLE mock_rules (                                  -- [existing] + chaos_mode [new]
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    token       TEXT NOT NULL,
    name        TEXT,
    priority    INTEGER NOT NULL DEFAULT 100,
    enabled     INTEGER NOT NULL DEFAULT 1,
    match_json  TEXT NOT NULL DEFAULT '{}',                -- serialized MatchCriteria
    response_json TEXT NOT NULL DEFAULT '{}',              -- serialized ResponseSpec
    state_writes_json TEXT NOT NULL DEFAULT '[]',          -- serialized StateWrite[]
    latency_ms  INTEGER,                                   -- null => inherit endpoint
    rate_limit_per_min INTEGER,                            -- null => inherit endpoint
    chaos_mode  TEXT,                                      -- [new] OQ-2: null => inherit endpoint
    webhook_json TEXT,                                     -- serialized WebhookAction | null
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (token) REFERENCES endpoints(token) ON DELETE CASCADE
);
CREATE INDEX idx_rules_token ON mock_rules(token, priority, id);

CREATE TABLE request_logs (                                -- [existing] (traces)
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    token           TEXT NOT NULL,
    method          TEXT NOT NULL,
    path            TEXT NOT NULL,
    status_code     INTEGER NOT NULL,
    served_by       TEXT NOT NULL,
    matched_rule_id INTEGER,
    duration_ms     INTEGER NOT NULL DEFAULT 0,
    overhead_ms     INTEGER NOT NULL DEFAULT 0,
    request_headers TEXT, query_params TEXT, request_body TEXT,
    response_headers TEXT, response_body TEXT,
    trace_json      TEXT, state_snapshot TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (token) REFERENCES endpoints(token) ON DELETE CASCADE
);
CREATE INDEX idx_logs_token_id ON request_logs(token, id DESC);
CREATE INDEX idx_logs_created ON request_logs(created_at);

-- NEW: replaces Redis hash state:<token> ------------------------------------
CREATE TABLE endpoint_state (                              -- [new]
    token      TEXT NOT NULL,
    key        TEXT NOT NULL,                              -- ^[A-Za-z0-9_-]{1,64}$ enforced before write
    value      TEXT NOT NULL,
    expires_at TEXT NOT NULL,                              -- now + STATE_TTL_SECONDS (24h); checked at read + sweep
    PRIMARY KEY (token, key),
    FOREIGN KEY (token) REFERENCES endpoints(token) ON DELETE CASCADE
);
CREATE INDEX idx_state_expires ON endpoint_state(expires_at);

-- NEW: replaces Redis list crud:<token>:<collection> ------------------------
CREATE TABLE crud_collections (                            -- [new]
    token      TEXT NOT NULL,
    name       TEXT NOT NULL,                              -- ^[A-Za-z0-9_-]{1,64}$
    items_json TEXT NOT NULL DEFAULT '[]',                 -- JSON array of objects, each with uuid "id"
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT NOT NULL,                              -- now + CRUD_TTL_SECONDS (24h)
    PRIMARY KEY (token, name),
    FOREIGN KEY (token) REFERENCES endpoints(token) ON DELETE CASCADE
);
CREATE INDEX idx_crud_expires ON crud_collections(expires_at);
