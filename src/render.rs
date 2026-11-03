use std::io::{self, Write};

use crate::ansi::{BOLD, Bg, CLEAR, Color, Fg, MoveTo, RESET, UNDERLINE};
use crate::app::{App, Screen};
use crate::highlight::syntax_color;

/// Draw one full frame for a terminal of the given size.
pub fn draw<W: Write>(out: &mut W, app: &App, width: u16, height: u16) -> io::Result<()> {
    write!(out, "{}{CLEAR}", MoveTo(0, 0))?;

    let content_width = width.saturating_sub(8).min(92);
    let left = width.saturating_sub(content_width) / 2;
    let title_y = 2;
    write!(
        out,
        "{}{}{BOLD}rusttype{RESET}{}  / {}{RESET}",
        MoveTo(left, title_y),
        Fg(Color::Cyan),
        Fg(Color::DarkGrey),
        app.title()
    )?;

    if app.screen == Screen::Results {
        write!(
            out,
            "{}{}{:.0} wpm   {:.1}% accuracy   {:.1}s{RESET}",
            MoveTo(left, title_y + 2),
            Fg(Color::Green),
            app.wpm(),
            app.accuracy(),
            app.elapsed().as_secs_f64()
        )?;
    }
    draw_code(out, app, left, title_y + 5, height.saturating_sub(10))?;

    write!(out, "{}", MoveTo(left, height.saturating_sub(2)))?;
    match app.screen {
        Screen::Results => write!(
            out,
            "{}{BOLD}complete!{RESET}{}  enter/n next   ctrl-r retry   esc quit",
            Fg(Color::Green),
            Fg(Color::DarkGrey)
        )?,
        Screen::Typing => write!(
            out,
            "{}{}",
            Fg(Color::DarkGrey),
            if app.autoclose() {
                "newlines, indents + closers are automatic   ctrl-r restart   esc quit"
            } else {
                "newlines + indentation are automatic   ctrl-r restart   esc quit"
            }
        )?,
    }
    write!(out, "{RESET}")?;
    out.flush()
}

fn draw_code<W: Write>(
    out: &mut W,
    app: &App,
    left: u16,
    top: u16,
    max_rows: u16,
) -> io::Result<()> {
    let target: Vec<char> = app.target().chars().collect();
    let mut row = 0_u16;
    let mut col = 0_u16;
    let mut line_no = 1;
    write!(out, "{}{}{line_no:>2}  ", MoveTo(left, top), Fg(Color::DarkGrey))?;

    for (index, ch) in target.iter().copied().enumerate() {
        if row >= max_rows {
            break;
        }
        if ch == '\n' {
            row += 1;
            col = 0;
            line_no += 1;
            if row < max_rows {
                write!(out, "{}{}{line_no:>2}  ", MoveTo(left, top + row), Fg(Color::DarkGrey))?;
            }
            continue;
        }
        write!(out, "{}", MoveTo(left + 4 + col, top + row))?;
        if let Some(actual) = app.typed.get(index) {
            if *actual == ch {
                write!(out, "{}{ch}", Fg(Color::Green))?;
            } else {
                let shown = if *actual == '\n' { '↵' } else { *actual };
                write!(out, "{}{UNDERLINE}{shown}{RESET}", Fg(Color::Red))?;
            }
        } else if index == app.typed.len() && app.screen == Screen::Typing {
            write!(out, "{}{}{ch}{RESET}", Fg(Color::Black), Bg(Color::Cyan))?;
        } else if app.auto_closed(&target, index) {
            // Already in place ahead of the cursor, as in an editor.
            write!(out, "{}{ch}", Fg(Color::Green))?;
        } else {
            write!(out, "{}{ch}", Fg(syntax_color(&target, index)))?;
        }
        col += 1;
    }
    write!(out, "{RESET}")
}
