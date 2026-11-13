mod app;
mod chain;
mod config;
mod dossier;
mod game;
mod kols;
mod llm;
mod model;
mod plan;
mod selftest;

use std::net::SocketAddr;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenv_lite();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "house=info,tower_http=warn".into()))
        .init();
    let cfg = config::Config::from_env()?;
    let g = game::Game::new(cfg)?;
    match std::env::args().nth(1).as_deref() {
        Some("selftest") => return selftest::run(g).await,
        Some("warm") => {
            for s in 0..g.cfg.slots {
                g.warm_slot(s).await;
            }
            println!("warmed {} slots", g.cfg.slots);
            return Ok(());
        }
        _ => {}
    }
    g.recover().await;
    let w = g.clone();
    tokio::spawn(async move {
        for s in 0..w.cfg.slots {
            w.warm_slot(s).await;
        }
    });
    tokio::spawn(g.clone().janitor());
    let addr = SocketAddr::from(([0, 0, 0, 0], g.cfg.port));
    tracing::info!(%addr, contract = %g.chain.contract, "CABAL house listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app::router(g).into_make_service_with_connect_info::<SocketAddr>()).await?;
    Ok(())
}

/// Load `KEY=VALUE` lines from `.env` (development only). Existing environment variables win.
fn dotenv_lite() -> Option<()> {
    for path in [".env", "../.env", "../../.env"] {
        if let Ok(s) = std::fs::read_to_string(path) {
            for line in s.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = line.split_once('=') {
                    if std::env::var(k.trim()).is_err() {
                        std::env::set_var(k.trim(), v.trim());
                    }
                }
            }
            return Some(());
        }
    }
    None
}
