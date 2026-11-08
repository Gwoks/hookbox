//! The trace write path over `request_logs` (AC-46/47/57): one INSERT plus
//! the write-time per-token cap prune, run off the response path.

use sqlx::SqlitePool;

/// A trace record to persist off the response path (AC-46/47/57). Bodies are
/// already truncated to `MAX_BODY_BYTES` and headers already redacted by the
/// engine before this struct is built.
#[derive(Debug, Clone)]
pub struct TraceRecord {
    pub token: String,
    pub method: String,
    pub path: String,
    pub status_code: i64,
    pub served_by: String,
    pub matched_rule_id: Option<i64>,
    pub duration_ms: i64,
    pub overhead_ms: i64,
    pub request_headers: String, // JSON
    pub query_params: String,    // JSON
    pub request_body: Option<String>,
    pub response_headers: String, // JSON
    pub response_body: Option<String>,
    pub trace_json: String,     // JSON array of {step, detail}
    pub state_snapshot: String, // JSON
}

/// Insert a trace, prune to the newest `trace_cap` rows for that token
/// (write-time prune so the per-endpoint cap never drifts — AC-48). Returns
/// the new row id. Runs off the response path (spawned by the engine, never
/// awaited) — every write here is fire-and-forget per AC-73(a).
///
/// The endpoint's lifetime counters (`request_count`/`last_hit`) are bumped
/// by the `trg_request_logs_bump_counters` AFTER INSERT trigger (migration
/// 0003), not by a separate `UPDATE` statement here: every traced request is
/// a real hit on a live endpoint (unknown-token 404s and tombstoned 410s
/// never reach `spawn_trace` — see `unknown_404_and_gone_410_not_traced`), so
/// the bump rides along with the INSERT below rather than costing this
/// function a third statement — the frozen AC-73(b)/AC-S20 baseline is TWO
/// (insert + prune). `request_count` is monotonic — it is never reset or
/// decremented — matching AC-77's premise, which F4 now publishes to the
/// public viewer (§5.5.4).
pub async fn insert_trace(
    pool: &SqlitePool,
    rec: &TraceRecord,
    trace_cap: i64,
) -> Result<i64, sqlx::Error> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO request_logs
            (token, method, path, status_code, served_by, matched_rule_id,
             duration_ms, overhead_ms, request_headers, query_params, request_body,
             response_headers, response_body, trace_json, state_snapshot)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?) RETURNING id",
    )
    .bind(&rec.token)
    .bind(&rec.method)
    .bind(&rec.path)
    .bind(rec.status_code)
    .bind(&rec.served_by)
    .bind(rec.matched_rule_id)
    .bind(rec.duration_ms)
    .bind(rec.overhead_ms)
    .bind(&rec.request_headers)
    .bind(&rec.query_params)
    .bind(&rec.request_body)
    .bind(&rec.response_headers)
    .bind(&rec.response_body)
    .bind(&rec.trace_json)
    .bind(&rec.state_snapshot)
    .fetch_one(pool)
    .await?;

    // Write-time prune: keep only the newest `trace_cap` rows for this token.
    sqlx::query(
        "DELETE FROM request_logs
         WHERE token = ?
           AND id NOT IN (
               SELECT id FROM request_logs WHERE token = ? ORDER BY id DESC LIMIT ?
           )",
    )
    .bind(&rec.token)
    .bind(&rec.token)
    .bind(trace_cap)
    .execute(pool)
    .await?;

    Ok(id)
}
