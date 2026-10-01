use std::sync::Arc;

use rusttype_core::{LeaderboardEntry, Submission, verify};

use crate::db::{Db, Outcome};

/// Replay the keystrokes, derive the score ourselves, then store it. The one
/// path every submission takes, whether it came over HTTP or an SSH session.
pub async fn submit(db: &Arc<Db>, user_id: i64, submission: Submission) -> Result<Outcome, String> {
    let verified = verify(&submission).map_err(|reject| reject.to_string())?;
    let db = Arc::clone(db);
    tokio::task::spawn_blocking(move || db.record_run(user_id, &verified))
        .await
        .map_err(|e| e.to_string())?
}

pub async fn leaderboard(db: &Arc<Db>, limit: u32) -> Vec<LeaderboardEntry> {
    let db = Arc::clone(db);
    tokio::task::spawn_blocking(move || db.leaderboard(limit).unwrap_or_default())
        .await
        .unwrap_or_default()
}

pub fn describe(outcome: &Outcome) -> String {
    match outcome.rank {
        Some(rank) if outcome.personal_best => format!("new personal best! leaderboard rank #{rank}"),
        Some(rank) => format!("saved. your best is rank #{rank}"),
        None => "saved, but under 90% accuracy so it is not ranked".to_string(),
    }
}
