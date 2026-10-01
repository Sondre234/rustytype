use serde::{Deserialize, Serialize};

/// Runs below this accuracy are stored but never ranked.
pub const MIN_RANKED_ACCURACY: f64 = 90.0;

/// What the server tells a player after saving a run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    pub wpm: f64,
    pub accuracy: f64,
    /// Overall rank, `None` when the accuracy was too low to be ranked.
    pub rank: Option<u32>,
    /// Rank on the snippet that was just typed.
    pub snippet_rank: Option<u32>,
    pub personal_best: bool,
    pub snippet_best: bool,
}

impl Outcome {
    pub fn describe(&self) -> String {
        let (Some(rank), Some(snippet_rank)) = (self.rank, self.snippet_rank) else {
            return format!(
                "saved, but under {MIN_RANKED_ACCURACY:.0}% accuracy so it is not ranked"
            );
        };
        let head = if self.personal_best {
            "new personal best!"
        } else if self.snippet_best {
            "new best on this snippet!"
        } else {
            "saved."
        };
        format!("{head} #{rank} overall, #{snippet_rank} on this snippet")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unranked_runs_explain_why_and_ranked_runs_show_both_ranks() {
        let mut outcome = Outcome {
            wpm: 90.0,
            accuracy: 80.0,
            rank: None,
            snippet_rank: None,
            personal_best: false,
            snippet_best: false,
        };
        assert!(outcome.describe().contains("not ranked"));
        outcome.accuracy = 99.0;
        outcome.rank = Some(3);
        outcome.snippet_rank = Some(1);
        outcome.snippet_best = true;
        assert_eq!(outcome.describe(), "new best on this snippet! #3 overall, #1 on this snippet");
    }
}
