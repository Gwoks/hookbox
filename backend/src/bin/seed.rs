//! `seed` bin — first-run demo-data seeding (scaffold placeholder).
//!
//! Fleshed out by hookbox-sks.24: opens the pool, migrates, and plants a demo
//! owner + endpoint + rules on first run (idempotent). The scaffold provides a
//! buildable entrypoint that migrates a fresh DB so the `[[bin]] seed` target
//! compiles and a fresh DB applies the §5.6 schema.

use hookbox::config::Config;
use hookbox::{db, seed};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::from_env();
    if cfg.sqlite_path != ":memory:" {
        if let Some(parent) = std::path::Path::new(&cfg.sqlite_path).parent() {
            std::fs::create_dir_all(parent).ok();
        }
    }
    let pool = db::pool(&cfg.sqlite_path).await?;
    db::migrate(&pool).await?;
    match seed::seed_if_empty(&pool, cfg.endpoint_id_length).await? {
        Some((email, secret, token)) => {
            eprintln!("hookbox seed — planted demo data:");
            eprintln!("  owner email : {email}");
            eprintln!("  owner secret: {secret}");
            eprintln!("  primary token: {token}");
        }
        None => eprintln!("hookbox seed — data already present; nothing to do"),
    }
    Ok(())
}
