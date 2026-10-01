use std::{
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use rand::RngCore;
use rusqlite::{Connection, OptionalExtension, params};
use rusttype_core::{LeaderboardEntry, Verified};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Runs below this accuracy are stored but never ranked.
const MIN_RANKED_ACCURACY: f64 = 90.0;
const SESSION_TTL_MS: i64 = 180 * 24 * 3600 * 1000;

#[derive(Clone, Debug)]
pub struct User {
    pub id: i64,
    pub login: String,
}

#[derive(Debug, Serialize)]
pub struct Outcome {
    pub wpm: f64,
    pub accuracy: f64,
    /// `None` when the accuracy was too low to be ranked.
    pub rank: Option<u32>,
    pub personal_best: bool,
}

pub struct Db(Mutex<Connection>);

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn hash_token(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl Db {
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS users (
                 id INTEGER PRIMARY KEY,
                 github_id INTEGER NOT NULL UNIQUE,
                 login TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS sessions (
                 token_hash TEXT PRIMARY KEY,
                 user_id INTEGER NOT NULL REFERENCES users(id),
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS runs (
                 id INTEGER PRIMARY KEY,
                 user_id INTEGER NOT NULL REFERENCES users(id),
                 snippet TEXT NOT NULL,
                 wpm REAL NOT NULL,
                 accuracy REAL NOT NULL,
                 elapsed_ms INTEGER NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS runs_by_user ON runs(user_id, wpm DESC);",
        )?;
        Ok(Self(Mutex::new(conn)))
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Create the user on first sight, and follow GitHub renames afterwards.
    pub fn upsert_user(&self, github_id: i64, login: &str) -> rusqlite::Result<User> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO users (github_id, login, created_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(github_id) DO UPDATE SET login = excluded.login",
            params![github_id, login, now_ms()],
        )?;
        let id = conn.query_row(
            "SELECT id FROM users WHERE github_id = ?1",
            [github_id],
            |row| row.get(0),
        )?;
        Ok(User { id, login: login.to_string() })
    }

    /// Returns the plaintext token once; only its hash is stored.
    pub fn create_session(&self, user_id: i64) -> rusqlite::Result<String> {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        let token: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        self.conn().execute(
            "INSERT INTO sessions (token_hash, user_id, created_at) VALUES (?1, ?2, ?3)",
            params![hash_token(&token), user_id, now_ms()],
        )?;
        Ok(token)
    }

    pub fn user_for_token(&self, token: &str) -> rusqlite::Result<Option<User>> {
        self.conn()
            .query_row(
                "SELECT u.id, u.login FROM sessions s JOIN users u ON u.id = s.user_id
                 WHERE s.token_hash = ?1 AND s.created_at > ?2",
                params![hash_token(token), now_ms() - SESSION_TTL_MS],
                |row| Ok(User { id: row.get(0)?, login: row.get(1)? }),
            )
            .optional()
    }

    /// Store a verified run. Refuses a user who submits faster than they could
    /// possibly have typed.
    pub fn record_run(&self, user_id: i64, run: &Verified) -> Result<Outcome, String> {
        let conn = self.conn();
        let now = now_ms();
        let last: Option<i64> = conn
            .query_row(
                "SELECT MAX(created_at) FROM runs WHERE user_id = ?1",
                [user_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if let Some(last) = last
            && ((now - last) as f64) < f64::from(run.elapsed_ms) * 0.9
        {
            return Err("submitting faster than the run took to type".into());
        }

        let best_before: Option<f64> = conn
            .query_row(
                "SELECT MAX(wpm) FROM runs WHERE user_id = ?1 AND accuracy >= ?2",
                params![user_id, MIN_RANKED_ACCURACY],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO runs (user_id, snippet, wpm, accuracy, elapsed_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![user_id, run.snippet, run.wpm, run.accuracy, run.elapsed_ms, now],
        )
        .map_err(|e| e.to_string())?;

        let ranked = run.accuracy >= MIN_RANKED_ACCURACY;
        let rank = if ranked {
            let best = best_before.map_or(run.wpm, |old| old.max(run.wpm));
            let ahead: u32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM (
                         SELECT MAX(wpm) AS best FROM runs
                         WHERE accuracy >= ?1 AND user_id != ?2
                         GROUP BY user_id HAVING best > ?3)",
                    params![MIN_RANKED_ACCURACY, user_id, best],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            Some(ahead + 1)
        } else {
            None
        };
        Ok(Outcome {
            wpm: run.wpm,
            accuracy: run.accuracy,
            rank,
            personal_best: ranked && best_before.is_none_or(|old| run.wpm > old),
        })
    }

    pub fn leaderboard(&self, limit: u32) -> rusqlite::Result<Vec<LeaderboardEntry>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT login, wpm, accuracy FROM (
                 SELECT u.login AS login, r.wpm AS wpm, r.accuracy AS accuracy,
                        ROW_NUMBER() OVER (PARTITION BY r.user_id ORDER BY r.wpm DESC) AS n
                 FROM runs r JOIN users u ON u.id = r.user_id
                 WHERE r.accuracy >= ?1)
             WHERE n = 1 ORDER BY wpm DESC LIMIT ?2",
        )?;
        statement
            .query_map(params![MIN_RANKED_ACCURACY, limit], |row| {
                Ok(LeaderboardEntry {
                    login: row.get(0)?,
                    wpm: row.get(1)?,
                    accuracy: row.get(2)?,
                })
            })?
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(wpm: f64, accuracy: f64) -> Verified {
        Verified { snippet: "s".into(), wpm, accuracy, elapsed_ms: 0 }
    }

    #[test]
    fn ranks_best_run_per_user_and_ignores_sloppy_runs() {
        let db = Db::open(":memory:").unwrap();
        let ada = db.upsert_user(1, "ada").unwrap();
        let bob = db.upsert_user(2, "bob").unwrap();

        assert_eq!(db.record_run(ada.id, &run(80.0, 99.0)).unwrap().rank, Some(1));
        let bob_first = db.record_run(bob.id, &run(60.0, 97.0)).unwrap();
        assert_eq!((bob_first.rank, bob_first.personal_best), (Some(2), true));
        // Fast but sloppy: stored, never ranked.
        assert_eq!(db.record_run(bob.id, &run(200.0, 70.0)).unwrap().rank, None);

        let board = db.leaderboard(10).unwrap();
        let order: Vec<_> = board.iter().map(|e| (e.login.as_str(), e.wpm)).collect();
        assert_eq!(order, [("ada", 80.0), ("bob", 60.0)]);
    }

    #[test]
    fn sessions_resolve_to_their_user_and_unknown_tokens_do_not() {
        let db = Db::open(":memory:").unwrap();
        let ada = db.upsert_user(1, "ada").unwrap();
        let token = db.create_session(ada.id).unwrap();
        assert_eq!(db.user_for_token(&token).unwrap().unwrap().login, "ada");
        assert!(db.user_for_token("nope").unwrap().is_none());
        // GitHub renames keep the same account.
        db.upsert_user(1, "ada-lovelace").unwrap();
        assert_eq!(db.user_for_token(&token).unwrap().unwrap().login, "ada-lovelace");
    }
}
