mod api;
mod login;

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
use rusttype_core::{Action, App, Key, draw};

const USAGE: &str = "usage: rusttype [login | logout] [--server URL]

  (no command)   practice typing Rust; scores are submitted if you are logged in
  login          sign in with GitHub (device flow) to appear on the leaderboard
  logout         forget the saved login

  --server URL   leaderboard server (default: $RUSTTYPE_SERVER, else the saved login's)";

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

fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let server_flag = args.iter().position(|arg| arg == "--server").map(|at| {
        args.remove(at);
        (at < args.len()).then(|| args.remove(at))
    });
    if matches!(server_flag, Some(None)) {
        anyhow::bail!("--server needs a URL\n\n{USAGE}");
    }
    let credentials = login::load();
    let server = server_flag
        .flatten()
        .or_else(|| std::env::var("RUSTTYPE_SERVER").ok())
        .or_else(|| credentials.as_ref().map(|saved| saved.server.clone()))
        .unwrap_or_else(|| api::DEFAULT_SERVER.to_string())
        .trim_end_matches('/')
        .to_string();

    match args.first().map(String::as_str) {
        Some("login") => login::login(&server),
        Some("logout") => login::logout(),
        Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            Ok(())
        }
        Some(other) => anyhow::bail!("unknown argument `{other}`\n\n{USAGE}"),
        None => Ok(play(&server, credentials.filter(|saved| saved.server == server))?),
    }
}

fn play(server: &str, credentials: Option<api::Credentials>) -> io::Result<()> {
    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen, ResetColor);
        previous_hook(info);
    }));

    let _guard = TerminalGuard::enter()?;
    let mut app = App::new();
    app.identity = credentials.as_ref().map(|saved| saved.login.clone());

    loop {
        render(&app)?;
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let Event::Key(key) = event::read()? else { continue };
        let Some(key) = map_key(key) else { continue };
        match app.handle_key(key, Instant::now()) {
            Action::None => {}
            Action::Quit => break,
            Action::Submit(submission) => {
                app.status = Some(match &credentials {
                    Some(credentials) => {
                        app.status = Some("submitting...".into());
                        render(&app)?;
                        match api::submit(credentials, &submission) {
                            Ok(outcome) => api::describe(&outcome),
                            Err(error) => format!("not saved: {}", api::explain(&error)),
                        }
                    }
                    None => "not saved. run `rusttype login` to join the leaderboard".into(),
                });
            }
            Action::ShowLeaderboard => match api::leaderboard(server) {
                Ok(entries) => app.show_leaderboard(entries),
                Err(error) => app.status = Some(format!("leaderboard unavailable: {}", api::explain(&error))),
            },
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
