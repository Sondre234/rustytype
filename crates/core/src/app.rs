use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::replay::{Keystroke, Stroke, Submission};
use crate::snippets::SNIPPETS;

/// Input, independent of whether it came from crossterm or an SSH byte stream.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Key {
    Char(char),
    Backspace,
    Enter,
    Restart,
    Tab,
    Quit,
}

/// What the host (local client or SSH session) must do after a key.
#[derive(Debug, PartialEq)]
pub enum Action {
    None,
    Quit,
    Submit(Submission),
    /// `None` = the global board; `Some(title)` = one snippet's board.
    ShowLeaderboard { snippet: Option<&'static str> },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Screen {
    Typing,
    Results,
    Leaderboard,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub login: String,
    pub wpm: f64,
    pub accuracy: f64,
    /// The snippet this run was typed on (a player's best run, for the global board).
    pub snippet: String,
}

pub struct App {
    snippet: usize,
    pub(crate) typed: Vec<char>,
    automatic: Vec<bool>,
    started: Option<Instant>,
    finished_in: Option<Duration>,
    log: Vec<Keystroke>,
    pub(crate) screen: Screen,
    /// Who is playing, shown in the header (set by the host).
    pub identity: Option<String>,
    /// One line under the results, e.g. the submission outcome.
    pub status: Option<String>,
    pub(crate) leaderboard: Vec<LeaderboardEntry>,
    /// Which board is showing: `None` = global, `Some(title)` = that snippet.
    pub(crate) board: Option<&'static str>,
    pub(crate) board_error: Option<String>,
}

impl App {
    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize;
        Self::for_snippet(seed % SNIPPETS.len())
    }

    pub fn for_snippet(snippet: usize) -> Self {
        Self {
            snippet,
            typed: Vec::new(),
            automatic: Vec::new(),
            started: None,
            finished_in: None,
            log: Vec::new(),
            screen: Screen::Typing,
            identity: None,
            status: None,
            leaderboard: Vec::new(),
            board: None,
            board_error: None,
        }
    }

    /// `snippet` is the board that `entries` belong to, as passed in the
    /// `Action::ShowLeaderboard` that asked for them.
    pub fn show_leaderboard(&mut self, snippet: Option<&'static str>, entries: Vec<LeaderboardEntry>) {
        self.board = snippet;
        self.leaderboard = entries;
        self.board_error = None;
        self.screen = Screen::Leaderboard;
    }

    /// The host could not fetch the board it was asked for; show why instead.
    pub fn leaderboard_failed(&mut self, snippet: Option<&'static str>, error: String) {
        self.board = snippet;
        self.leaderboard.clear();
        self.board_error = Some(error);
        self.screen = Screen::Leaderboard;
    }

    pub fn title(&self) -> &'static str {
        SNIPPETS[self.snippet].0
    }

    pub(crate) fn target(&self) -> &'static str {
        SNIPPETS[self.snippet].1
    }

    fn target_chars(&self) -> Vec<char> {
        self.target().chars().collect()
    }

    fn reset(&mut self, next: bool) {
        if next {
            self.snippet = (self.snippet + 1) % SNIPPETS.len();
        }
        self.typed.clear();
        self.automatic.clear();
        self.started = None;
        self.finished_in = None;
        self.log.clear();
        self.status = None;
        self.screen = Screen::Typing;
    }

    pub fn handle_key(&mut self, key: Key, now: Instant) -> Action {
        match key {
            Key::Quit => return Action::Quit,
            Key::Restart => {
                self.reset(false);
                return Action::None;
            }
            _ => {}
        }
        match self.screen {
            Screen::Leaderboard if key == Key::Tab => Action::ShowLeaderboard {
                snippet: match self.board {
                    Some(_) => None,
                    None => Some(self.title()),
                },
            },
            Screen::Leaderboard => {
                self.screen = Screen::Results;
                Action::None
            }
            Screen::Results => match key {
                Key::Enter | Key::Char('n') | Key::Char(' ') => {
                    self.reset(true);
                    Action::None
                }
                Key::Char('l') => Action::ShowLeaderboard { snippet: None },
                _ => Action::None,
            },
            Screen::Typing => match key {
                Key::Backspace => {
                    self.backspace(now);
                    Action::None
                }
                Key::Char(ch) => self.push(ch, now),
                _ => Action::None,
            },
        }
    }

    fn push(&mut self, ch: char, now: Instant) -> Action {
        if self.typed.len() >= self.target().chars().count() {
            return Action::None;
        }
        let started = *self.started.get_or_insert(now);
        self.log.push(Keystroke {
            ms: now.saturating_duration_since(started).as_millis() as u32,
            stroke: Stroke::Char(ch),
        });
        self.typed.push(ch);
        self.automatic.push(false);
        self.advance_layout();
        if self.typed.len() == self.target().chars().count() {
            self.finished_in = Some(now.saturating_duration_since(started));
            self.screen = Screen::Results;
            return Action::Submit(Submission {
                snippet: self.title().to_string(),
                strokes: self.log.clone(),
            });
        }
        Action::None
    }

    fn backspace(&mut self, now: Instant) {
        while self.automatic.last() == Some(&true) {
            self.typed.pop();
            self.automatic.pop();
        }
        self.typed.pop();
        self.automatic.pop();
        match self.started {
            Some(started) if !self.typed.is_empty() => self.log.push(Keystroke {
                ms: now.saturating_duration_since(started).as_millis() as u32,
                stroke: Stroke::Backspace,
            }),
            _ => {
                self.started = None;
                self.log.clear();
            }
        }
    }

    /// Newlines and leading indentation are typed for the player.
    fn advance_layout(&mut self) {
        let target = self.target_chars();
        while self.typed.len() < target.len() {
            let index = self.typed.len();
            let at_line_start = index == 0 || target[index - 1] == '\n';
            if target[index] == '\n' {
                self.typed.push('\n');
                self.automatic.push(true);
            } else if at_line_start && target[index] == ' ' {
                while self.typed.len() < target.len() && target[self.typed.len()] == ' ' {
                    self.typed.push(' ');
                    self.automatic.push(true);
                }
            } else {
                break;
            }
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.finished_in
            .or_else(|| self.started.map(|start| start.elapsed()))
            .unwrap_or_default()
    }

    fn correct(&self) -> usize {
        self.typed
            .iter()
            .zip(self.target().chars())
            .enumerate()
            .filter(|(index, _)| !self.automatic[*index])
            .filter(|(_, (actual, expected))| actual == &expected)
            .count()
    }

    fn user_keystrokes(&self) -> usize {
        self.automatic.iter().filter(|&&automatic| !automatic).count()
    }

    pub fn accuracy(&self) -> f64 {
        let keystrokes = self.user_keystrokes();
        if keystrokes == 0 {
            100.0
        } else {
            self.correct() as f64 / keystrokes as f64 * 100.0
        }
    }

    pub fn wpm(&self) -> f64 {
        let minutes = self.elapsed().as_secs_f64() / 60.0;
        if minutes == 0.0 {
            0.0
        } else {
            self.correct() as f64 / 5.0 / minutes
        }
    }

    pub(crate) fn next_expected_char(&self) -> Option<char> {
        self.target().chars().nth(self.typed.len())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_count_only_correct_characters() {
        let mut app = App::for_snippet(0);
        let now = Instant::now();
        for ch in app.target().chars().take(4).collect::<Vec<_>>() {
            app.push(if ch == 'u' { 'x' } else { ch }, now);
        }
        assert_eq!(app.accuracy(), 75.0);
    }

    #[test]
    fn newlines_and_indentation_are_automatic() {
        let mut app = App::for_snippet(0);
        let now = Instant::now();
        let first_line = app.target().find('\n').unwrap();
        for ch in app.target().chars().take(first_line).collect::<Vec<_>>() {
            app.push(ch, now);
        }
        assert!(app.typed.ends_with(&['\n', ' ', ' ', ' ', ' ']));
    }

    #[test]
    fn tab_on_the_leaderboard_flips_between_global_and_this_snippet() {
        let mut app = App::for_snippet(0);
        let (now, title) = (Instant::now(), app.title());
        app.show_leaderboard(None, vec![]);
        assert_eq!(
            app.handle_key(Key::Tab, now),
            Action::ShowLeaderboard { snippet: Some(title) }
        );
        app.show_leaderboard(Some(title), vec![]);
        assert_eq!(app.handle_key(Key::Tab, now), Action::ShowLeaderboard { snippet: None });
        // Anything else leaves the leaderboard.
        assert_eq!(app.handle_key(Key::Enter, now), Action::None);
        assert_eq!(app.screen, Screen::Results);
    }

    #[test]
    fn backspacing_everything_resets_the_timeline() {
        let mut app = App::for_snippet(0);
        let now = Instant::now();
        app.handle_key(Key::Char('p'), now);
        app.handle_key(Key::Backspace, now);
        assert!(app.log.is_empty() && app.started.is_none());
    }
}
