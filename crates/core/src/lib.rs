//! Game logic: snippets, typing state, and rendering to any `io::Write`.

mod app;
mod highlight;
mod pairs;
mod render;
mod snippets;

pub use app::{Action, App, Key, Screen};
pub use render::draw;
pub use snippets::SNIPPETS;
