use std::time::Duration;

use anyhow::bail;
use rusttype_core::{LeaderboardEntry, Submission};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Where scores go unless `--server`, `$RUSTTYPE_SERVER` or a saved login says otherwise.
pub const DEFAULT_SERVER: &str = "http://localhost:8080";

#[derive(Serialize, Deserialize)]
pub struct Credentials {
    pub server: String,
    pub token: String,
    pub login: String,
}

#[derive(Deserialize)]
pub struct Outcome {
    pub rank: Option<u32>,
    pub personal_best: bool,
}

#[derive(Deserialize)]
struct ServerError {
    error: String,
}

pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        // Read the server's JSON error message instead of a bare status error.
        .http_status_as_error(false)
        .build()
        .into()
}

fn parse<T: DeserializeOwned>(mut response: ureq::http::Response<ureq::Body>) -> anyhow::Result<T> {
    if response.status().is_success() {
        return Ok(response.body_mut().read_json()?);
    }
    let status = response.status();
    match response.body_mut().read_json::<ServerError>() {
        Ok(error) => bail!("{}", error.error),
        Err(_) => bail!("server returned {status}"),
    }
}

pub fn github_client_id(server: &str) -> anyhow::Result<String> {
    #[derive(Deserialize)]
    struct Config {
        github_client_id: String,
    }
    let config: Config = parse(agent().get(format!("{server}/api/config")).call()?)?;
    Ok(config.github_client_id)
}

pub fn exchange_github_token(server: &str, access_token: &str) -> anyhow::Result<Credentials> {
    #[derive(Deserialize)]
    struct Session {
        token: String,
        login: String,
    }
    let session: Session = parse(
        agent()
            .post(format!("{server}/api/auth/github"))
            .send_json(serde_json::json!({ "access_token": access_token }))?,
    )?;
    Ok(Credentials { server: server.to_string(), token: session.token, login: session.login })
}

pub fn submit(credentials: &Credentials, submission: &Submission) -> anyhow::Result<Outcome> {
    parse(
        agent()
            .post(format!("{}/api/scores", credentials.server))
            .header("Authorization", format!("Bearer {}", credentials.token))
            .send_json(submission)?,
    )
}

pub fn leaderboard(server: &str) -> anyhow::Result<Vec<LeaderboardEntry>> {
    parse(agent().get(format!("{server}/api/leaderboard?limit=15")).call()?)
}

pub fn describe(outcome: &Outcome) -> String {
    match (outcome.rank, outcome.personal_best) {
        (Some(rank), true) => format!("new personal best! leaderboard rank #{rank}"),
        (Some(rank), false) => format!("saved. your best is rank #{rank}"),
        (None, _) => "saved, but under 90% accuracy so it is not ranked".to_string(),
    }
}

/// Keep the context for a failed request on one line.
pub fn explain(error: &anyhow::Error) -> String {
    error.root_cause().to_string().lines().next().unwrap_or("request failed").to_string()
}

