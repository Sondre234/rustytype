use std::{
    fmt,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use crate::app::{Action, App, Key};
use crate::snippets;

/// Fastest believable run; anything above is rejected as scripted.
pub const MAX_WPM: f64 = 250.0;
/// Fraction of strokes allowed to land within `BURST_GAP_MS` of the previous one.
const BURST_GAP_MS: u32 = 8;
const MAX_BURST_SHARE: f64 = 0.05;
const MAX_STROKES: usize = 6000;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stroke {
    Char(char),
    Backspace,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keystroke {
    /// Milliseconds since the first keystroke of the attempt.
    pub ms: u32,
    pub stroke: Stroke,
}

/// A finished attempt, as the full keystroke timeline. Clients never claim a
/// score; the server replays this and derives it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Submission {
    pub snippet: String,
    pub strokes: Vec<Keystroke>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Verified {
    pub snippet: String,
    pub wpm: f64,
    pub accuracy: f64,
    pub elapsed_ms: u32,
}

#[derive(Debug)]
pub struct Reject(pub &'static str);

impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

pub fn verify(submission: &Submission) -> Result<Verified, Reject> {
    let index = snippets::index_of(&submission.snippet).ok_or(Reject("unknown snippet"))?;
    let strokes = &submission.strokes;
    if strokes.is_empty() || strokes.len() > MAX_STROKES {
        return Err(Reject("bad keystroke count"));
    }
    if strokes[0].ms != 0 || strokes.windows(2).any(|pair| pair[1].ms < pair[0].ms) {
        return Err(Reject("timeline is not monotonic"));
    }

    let mut app = App::for_snippet(index);
    let base = Instant::now();
    let mut finished = false;
    for keystroke in strokes {
        if finished {
            return Err(Reject("keystrokes after completion"));
        }
        let key = match keystroke.stroke {
            Stroke::Char(ch) => Key::Char(ch),
            Stroke::Backspace => Key::Backspace,
        };
        let now = base + Duration::from_millis(u64::from(keystroke.ms));
        finished = matches!(app.handle_key(key, now), Action::Submit(_));
    }
    if !finished {
        return Err(Reject("snippet was not completed"));
    }

    let bursts = strokes
        .windows(2)
        .filter(|pair| pair[1].ms - pair[0].ms < BURST_GAP_MS)
        .count();
    if bursts as f64 > strokes.len() as f64 * MAX_BURST_SHARE {
        return Err(Reject("keystrokes arrive too fast to be typed"));
    }
    let wpm = app.wpm();
    if wpm > MAX_WPM {
        return Err(Reject("speed is not humanly plausible"));
    }
    Ok(Verified {
        snippet: submission.snippet.clone(),
        wpm,
        accuracy: app.accuracy(),
        elapsed_ms: app.elapsed().as_millis() as u32,
    })
}


/// A perfect run of `title` typed with a constant gap between keystrokes.
/// Meant for tests and tooling that need a valid `Submission`.
pub fn synthetic_run(title: &str, gap_ms: u32) -> Option<Submission> {
    let mut app = App::for_snippet(snippets::index_of(title)?);
    let base = Instant::now();
    let mut strokes = Vec::new();
    let mut ms = 0;
    loop {
        let ch = app.next_expected_char()?;
        strokes.push(Keystroke { ms, stroke: Stroke::Char(ch) });
        let now = base + Duration::from_millis(u64::from(ms));
        if matches!(app.handle_key(Key::Char(ch), now), Action::Submit(_)) {
            return Some(Submission { snippet: title.into(), strokes });
        }
        ms += gap_ms;
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recorded_run_replays_to_the_same_score() {
        let title = snippets::SNIPPETS[0].0;
        let verified = verify(&synthetic_run(title, 120).unwrap()).unwrap();
        assert_eq!(verified.accuracy, 100.0);
        assert!(verified.wpm > 80.0 && verified.wpm < 130.0, "{}", verified.wpm);
    }

    #[test]
    fn scripted_speed_is_rejected() {
        let title = snippets::SNIPPETS[0].0;
        assert!(verify(&synthetic_run(title, 1).unwrap()).is_err());
        assert!(verify(&synthetic_run(title, 30).unwrap()).is_err());
    }

    #[test]
    fn incomplete_or_unknown_runs_are_rejected() {
        let title = snippets::SNIPPETS[0].0;
        let mut run = synthetic_run(title, 120).unwrap();
        run.strokes.pop();
        assert!(verify(&run).is_err());
        run.snippet = "nope".into();
        assert!(verify(&run).is_err());
    }
}
