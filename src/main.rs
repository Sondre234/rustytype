mod ansi;
mod app;
mod highlight;
mod pairs;
mod render;
mod snippets;
mod tty;

use std::{
    io,
    time::{Duration, Instant},
};

use app::{Action, App};
use render::draw;
use tty::Terminal;

const USAGE: &str = "usage: rusttype [--classic]

  --classic   type closing brackets and quotes yourself (default: they fill in)";

fn main() -> io::Result<()> {
    let mut autoclose = true;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--classic" => autoclose = false,
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => {
                eprintln!("unknown argument `{other}`\n\n{USAGE}");
                std::process::exit(2);
            }
        }
    }
    play(autoclose)
}

fn play(autoclose: bool) -> io::Result<()> {
    let mut terminal = Terminal::enter()?;
    let mut app = App::new(autoclose);
    let mut frame = Vec::new();
    // The terminal size the frame on screen was drawn for; `None` once a key
    // has changed what it should show.
    let mut drawn_for = None;
    loop {
        let (width, height) = terminal.size()?;
        if drawn_for != Some((width, height)) {
            frame.clear();
            draw(&mut frame, &app, width, height)?;
            terminal.show(&frame)?;
            drawn_for = Some((width, height));
        }
        // Waking up every so often picks up a resize.
        if let Some(key) = terminal.read_key(Duration::from_millis(100))? {
            if app.handle_key(key, Instant::now()) == Action::Quit {
                return Ok(());
            }
            drawn_for = None;
        }
    }
}
