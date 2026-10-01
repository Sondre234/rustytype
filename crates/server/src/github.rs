use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use russh::keys::PublicKey;
use serde::Deserialize;

const CACHE_TTL: Duration = Duration::from_secs(600);
const USER_AGENT: &str = "rusttype-server";

#[derive(Clone, Debug)]
pub struct GithubUser {
    pub id: i64,
    pub login: String,
}

#[derive(Deserialize)]
struct ApiUser {
    id: i64,
    login: String,
}

#[derive(Deserialize)]
struct TokenCheck {
    user: ApiUser,
}

type CacheEntry = (Instant, GithubUser, Vec<PublicKey>);

pub struct Github {
    http: reqwest::Client,
    pub client_id: String,
    client_secret: String,
    /// login (lowercase) -> (when fetched, id, keys)
    cache: Mutex<HashMap<String, CacheEntry>>,
}

/// GitHub logins are 1-39 chars of [A-Za-z0-9-]. Validating before building a
/// URL keeps untrusted SSH usernames out of the request path.
pub fn is_valid_login(login: &str) -> bool {
    !login.is_empty()
        && login.len() <= 39
        && login.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

impl Github {
    pub fn new(client_id: String, client_secret: String) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(10))
            .build()
            .expect("reqwest client");
        Self { http, client_id, client_secret, cache: Mutex::new(HashMap::new()) }
    }

    /// Check that `access_token` was issued to *our* OAuth app (not some other
    /// app that happens to hold a token for the same person) and return who it
    /// belongs to.
    pub async fn user_for_access_token(&self, access_token: &str) -> Option<GithubUser> {
        let url = format!(
            "https://api.github.com/applications/{}/token",
            self.client_id
        );
        let response = self
            .http
            .post(url)
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({ "access_token": access_token }))
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let check = response.json::<TokenCheck>().await.ok()?;
        Some(GithubUser { id: check.user.id, login: check.user.login })
    }

    /// The account's id plus its public SSH keys, cached for a few minutes.
    pub async fn identity_and_keys(&self, login: &str) -> Option<(GithubUser, Vec<PublicKey>)> {
        if !is_valid_login(login) {
            return None;
        }
        let cache_key = login.to_ascii_lowercase();
        if let Some((fetched, user, keys)) = self.cache.lock().ok()?.get(&cache_key)
            && fetched.elapsed() < CACHE_TTL
        {
            return Some((user.clone(), keys.clone()));
        }

        let user = self
            .http
            .get(format!("https://api.github.com/users/{login}"))
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json::<ApiUser>()
            .await
            .ok()?;
        let text = self
            .http
            .get(format!("https://github.com/{login}.keys"))
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .text()
            .await
            .ok()?;
        let keys: Vec<PublicKey> = text
            .lines()
            .filter_map(|line| PublicKey::from_openssh(line.trim()).ok())
            .collect();
        let user = GithubUser { id: user.id, login: user.login };
        self.cache
            .lock()
            .ok()?
            .insert(cache_key, (Instant::now(), user.clone(), keys.clone()));
        Some((user, keys))
    }
}

#[cfg(test)]
mod tests {
    use super::is_valid_login;

    #[test]
    fn only_plausible_github_logins_are_looked_up() {
        assert!(is_valid_login("Sondre234"));
        assert!(is_valid_login("a-b"));
        assert!(!is_valid_login(""));
        assert!(!is_valid_login("../../etc"));
        assert!(!is_valid_login("a b"));
        assert!(!is_valid_login(&"x".repeat(40)));
    }
}
