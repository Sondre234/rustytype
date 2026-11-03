mod app;
mod highlight;
mod pairs;
mod render;
mod snippets;

use std::{
    io::{self, Write},
    panic,
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    style::ResetColor,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use app::{Action, App, Key};
use render::draw;

const USAGE: &str = "usage: rusttype [--classic]

  --classic   type closing brackets and quotes yourself (default: they fill in)";

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen, ResetColor);
    }
}

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
    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen, ResetColor);
        previous_hook(info);
    }));

    let _guard = TerminalGuard::enter()?;
    let mut app = App::new(autoclose);

    loop {
        render(&app)?;
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let Event::Key(key) = event::read()? else { continue };
        let Some(key) = map_key(key) else { continue };
        if app.handle_key(key, Instant::now()) == Action::Quit {
            break;
        }
    }
    Ok(())
}

fn render(app: &App) -> io::Result<()> {
    let (width, height) = terminal::size()?;
    let mut out = io::stdout().lock();
    draw(&mut out, app, width, height)?;
    out.flush()
}

fn map_key(key: KeyEvent) -> Option<Key> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => Some(Key::Quit),
        KeyCode::Char('c') if ctrl => Some(Key::Quit),
        KeyCode::Char('r') if ctrl => Some(Key::Restart),
        KeyCode::Char(ch) if !ctrl => Some(Key::Char(ch)),
        KeyCode::Backspace => Some(Key::Backspace),
        KeyCode::Enter => Some(Key::Enter),
        _ => None,
    }
}
