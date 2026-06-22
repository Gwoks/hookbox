//! `hookbox` bin entrypoint: load Config, open the WAL SQLite pool, migrate on
//! startup (AC-53), build the in-process components into `AppState`, and serve.
//!
//! Scaffold (hookbox-sks.11): serves a minimal `/healthz` so the binary runs
//! end-to-end against a migrated DB. The plane router + the 18 `/api` routes +
//! interceptor are wired in by tasks sks.13 / sks.14 (which replace
//! `scaffold_router` with the real `router::build_app(state)`).

use hookbox::config::Config;
use hookbox::state::AppState;
use hookbox::tasks::sweep;
use hookbox::{db, router, seed};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cfg = Config::from_env();

    if cfg.path_fallback_only {
        tracing::warn!(
            "MOCK_DOMAIN is unset/misconfigured — serving path-fallback-only mode"
        );
    }

    // Ensure the SQLite parent directory exists before opening.
    if cfg.sqlite_path != ":memory:" {
        if let Some(parent) = std::path::Path::new(&cfg.sqlite_path).parent() {
            std::fs::create_dir_all(parent).ok();
        }
    }

    let pool = db::pool(&cfg.sqlite_path).await?;
    db::migrate(&pool).await?; // migrate-on-startup (AC-53)

    // First-run demo data (AC-54): idempotent — a no-op once any endpoint exists.
    let id_len = cfg.endpoint_id_length;
    match seed::seed_if_empty(&pool, id_len).await {
        Ok(Some((email, _secret, token))) => {
            tracing::info!("seeded demo data: owner {email}, primary endpoint {token}");
        }
        Ok(None) => {}
        Err(e) => tracing::warn!("seed skipped ({e})"),
    }

    let bind_host = cfg.bind_host.clone();
    let port = cfg.public_port;
    let state = AppState::new(pool, cfg);

    // Background retention sweep (traces/state/crud/tombstones + bucket eviction).
    let _sweep = sweep::spawn(state.pool.clone(), state.cfg.clone(), state.limiter.clone());

    // Top-level plane dispatch (P1 interceptor / P2 /api / P3 SPA+feed+health).
    let app = router::build_app(state);

    let addr = format!("{bind_host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("hookbox listening on {addr}");
    // ConnectInfo is needed by the per-IP session rate limiter (§5.2 #1).
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}
