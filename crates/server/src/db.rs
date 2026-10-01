use std::{
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use rand::RngCore;
use rusqlite::{Connection, OptionalExtension, params};
use rusttype_core::{LeaderboardEntry, MIN_RANKED_ACCURACY, Outcome, Verified};
use sha2::{Digest, Sha256};

const SESSION_TTL_MS: i64 = 180 * 24 * 3600 * 1000;

#[derive(Clone, Debug)]
pub struct User {
    pub id: i64,
    pub login: String,
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
             CREATE INDEX IF NOT EXISTS runs_by_user ON runs(user_id, wpm DESC);
             CREATE INDEX IF NOT EXISTS runs_by_snippet ON runs(snippet, wpm DESC);",
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

    /// A user's best ranked wpm, overall or on one snippet.
    fn best_wpm(conn: &Connection, user_id: i64, snippet: Option<&str>) -> Result<Option<f64>, String> {
        conn.query_row(
            "SELECT MAX(wpm) FROM runs
             WHERE user_id = ?1 AND accuracy >= ?2 AND (?3 IS NULL OR snippet = ?3)",
            params![user_id, MIN_RANKED_ACCURACY, snippet],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())
    }

    /// 1 + the number of other players whose best ranked run beats `best`.
    fn rank_for(conn: &Connection, user_id: i64, best: f64, snippet: Option<&str>) -> Result<u32, String> {
        let ahead: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM (
                     SELECT MAX(wpm) AS best FROM runs
                     WHERE accuracy >= ?1 AND user_id != ?2 AND (?4 IS NULL OR snippet = ?4)
                     GROUP BY user_id HAVING best > ?3)",
                params![MIN_RANKED_ACCURACY, user_id, best, snippet],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(ahead + 1)
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

        let snippet = Some(run.snippet.as_str());
        let overall_before = Self::best_wpm(&conn, user_id, None)?;
        let snippet_before = Self::best_wpm(&conn, user_id, snippet)?;
        conn.execute(
            "INSERT INTO runs (user_id, snippet, wpm, accuracy, elapsed_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![user_id, run.snippet, run.wpm, run.accuracy, run.elapsed_ms, now],
        )
        .map_err(|e| e.to_string())?;

        let ranked = run.accuracy >= MIN_RANKED_ACCURACY;
        let (rank, snippet_rank) = if ranked {
            let overall = overall_before.map_or(run.wpm, |old| old.max(run.wpm));
            let on_snippet = snippet_before.map_or(run.wpm, |old| old.max(run.wpm));
            (
                Some(Self::rank_for(&conn, user_id, overall, None)?),
                Some(Self::rank_for(&conn, user_id, on_snippet, snippet)?),
            )
        } else {
            (None, None)
        };
        Ok(Outcome {
            wpm: run.wpm,
            accuracy: run.accuracy,
            rank,
            snippet_rank,
            personal_best: ranked && overall_before.is_none_or(|old| run.wpm > old),
            snippet_best: ranked && snippet_before.is_none_or(|old| run.wpm > old),
        })
    }

    /// Each player's best ranked run, best first. `snippet = None` is the
    /// global board (each row carries the snippet that run was on).
    pub fn leaderboard(&self, snippet: Option<&str>, limit: u32) -> rusqlite::Result<Vec<LeaderboardEntry>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT login, wpm, accuracy, snippet FROM (
                 SELECT u.login AS login, r.wpm AS wpm, r.accuracy AS accuracy,
                        r.snippet AS snippet,
                        ROW_NUMBER() OVER (
                            PARTITION BY r.user_id ORDER BY r.wpm DESC, r.created_at
                        ) AS n
                 FROM runs r JOIN users u ON u.id = r.user_id
                 WHERE r.accuracy >= ?1 AND (?3 IS NULL OR r.snippet = ?3))
             WHERE n = 1 ORDER BY wpm DESC, login LIMIT ?2",
        )?;
        statement
            .query_map(params![MIN_RANKED_ACCURACY, limit, snippet], |row| {
                Ok(LeaderboardEntry {
                    login: row.get(0)?,
                    wpm: row.get(1)?,
                    accuracy: row.get(2)?,
                    snippet: row.get(3)?,
                })
            })?
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(snippet: &str, wpm: f64, accuracy: f64) -> Verified {
        Verified { snippet: snippet.into(), wpm, accuracy, elapsed_ms: 0 }
    }

    fn board(db: &Db, snippet: Option<&str>) -> Vec<(String, f64, String)> {
        db.leaderboard(snippet, 10)
            .unwrap()
            .into_iter()
            .map(|e| (e.login, e.wpm, e.snippet))
            .collect()
    }

    #[test]
    fn global_board_shows_each_players_best_snippet_and_snippet_boards_filter() {
        let db = Db::open(":memory:").unwrap();
        let ada = db.upsert_user(1, "ada").unwrap();
        let bob = db.upsert_user(2, "bob").unwrap();

        let first = db.record_run(ada.id, &run("a", 80.0, 99.0)).unwrap();
        assert_eq!((first.rank, first.snippet_rank, first.personal_best), (Some(1), Some(1), true));
        db.record_run(ada.id, &run("b", 70.0, 98.0)).unwrap();
        // Bob is slower than ada overall but wins snippet "b".
        let bob_run = db.record_run(bob.id, &run("b", 75.0, 97.0)).unwrap();
        assert_eq!((bob_run.rank, bob_run.snippet_rank), (Some(2), Some(1)));
        // Fast but sloppy: stored, never ranked.
        assert_eq!(db.record_run(bob.id, &run("a", 200.0, 70.0)).unwrap().rank, None);

        assert_eq!(
            board(&db, None),
            [("ada".into(), 80.0, "a".into()), ("bob".into(), 75.0, "b".into())]
        );
        assert_eq!(
            board(&db, Some("b")),
            [("bob".into(), 75.0, "b".into()), ("ada".into(), 70.0, "b".into())]
        );
        assert_eq!(board(&db, Some("a")), [("ada".into(), 80.0, "a".into())]);
        assert!(board(&db, Some("never-typed")).is_empty());
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
