//! rusttype server: leaderboard HTTP API plus an SSH front-end that serves the
//! game itself. Both authenticate against GitHub and share one SQLite file.

mod db;
mod github;
mod http;
mod service;
mod ssh;

use std::{path::PathBuf, sync::Arc};

use anyhow::Context;

pub struct AppState {
    pub db: Arc<db::Db>,
    pub github: github::Github,
    /// Hostname shown in connect instructions.
    pub public_host: String,
}

fn env(name: &str) -> anyhow::Result<String> {
    std::env::var(name).with_context(|| format!("{name} must be set"))
}

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let data_dir = PathBuf::from(env_or("RUSTTYPE_DATA_DIR", "."));
    std::fs::create_dir_all(&data_dir)?;
    let db = db::Db::open(data_dir.join("rusttype.db").to_str().context("data dir path")?)?;
    let state = Arc::new(AppState {
        db: Arc::new(db),
        github: github::Github::new(env("GITHUB_CLIENT_ID")?, env("GITHUB_CLIENT_SECRET")?),
        public_host: env_or("RUSTTYPE_PUBLIC_HOST", "localhost"),
    });

    let http_addr = env_or("RUSTTYPE_HTTP_ADDR", "127.0.0.1:8080");
    let ssh_addr = env_or("RUSTTYPE_SSH_ADDR", "0.0.0.0:2222");
    let listener = tokio::net::TcpListener::bind(&http_addr).await?;
    eprintln!("http on {http_addr}, ssh on {ssh_addr}");

    let host_key_path = data_dir.join("ssh_host_ed25519_key");
    let ssh = ssh::serve(Arc::clone(&state), ssh_addr, &host_key_path);
    let http = axum::serve(listener, http::router(state));
    tokio::select! {
        result = ssh => result,
        result = http => Ok(result?),
    }
}
