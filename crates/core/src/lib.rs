//! Game logic shared by the local client and the SSH server: snippets, typing
//! state, rendering to any `io::Write`, and server-side replay verification.

mod app;
mod highlight;
mod render;
mod replay;
mod snippets;

pub use app::{Action, App, Key, LeaderboardEntry, Screen};
pub use render::draw;
pub use replay::{Keystroke, MAX_WPM, Reject, Stroke, Submission, Verified, synthetic_run, verify};
pub use snippets::SNIPPETS;
