use std::sync::Arc;

use rusttype_core::{LeaderboardEntry, Outcome, Submission, verify};

use crate::db::Db;

/// Replay the keystrokes, derive the score ourselves, then store it. The one
/// path every submission takes, whether it came over HTTP or an SSH session.
pub async fn submit(db: &Arc<Db>, user_id: i64, submission: Submission) -> Result<Outcome, String> {
    let verified = verify(&submission).map_err(|reject| reject.to_string())?;
    let db = Arc::clone(db);
    tokio::task::spawn_blocking(move || db.record_run(user_id, &verified))
        .await
        .map_err(|e| e.to_string())?
}

/// `snippet = None` is the global board.
pub async fn leaderboard(db: &Arc<Db>, snippet: Option<String>, limit: u32) -> Vec<LeaderboardEntry> {
    let db = Arc::clone(db);
    tokio::task::spawn_blocking(move || db.leaderboard(snippet.as_deref(), limit).unwrap_or_default())
        .await
        .unwrap_or_default()
}
