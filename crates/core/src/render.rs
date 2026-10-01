use std::io::{self, Write};

use crossterm::{
    cursor::MoveTo,
    queue,
    style::{
        Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor,
        SetForegroundColor,
    },
    terminal::{Clear, ClearType},
};

use crate::app::{App, Screen};
use crate::highlight::syntax_color;

const LEADERBOARD_ROWS: usize = 15;

/// Draw one full frame. Takes the terminal size explicitly so the same code
/// serves a local tty and a remote SSH pty.
pub fn draw<W: Write>(out: &mut W, app: &App, width: u16, height: u16) -> io::Result<()> {
    queue!(out, MoveTo(0, 0), Clear(ClearType::All))?;

    let content_width = width.saturating_sub(8).min(92);
    let left = width.saturating_sub(content_width) / 2;
    let title_y = 2;
    queue!(
        out,
        MoveTo(left, title_y),
        SetForegroundColor(Color::Cyan),
        SetAttribute(Attribute::Bold),
        Print("rusttype"),
        SetAttribute(Attribute::Reset),
        SetForegroundColor(Color::DarkGrey),
    )?;
    if app.screen == Screen::Leaderboard {
        queue!(out, Print("  / leaderboard"))?;
    } else {
        queue!(out, Print(format!("  / {}", app.title())))?;
    }
    if let Some(identity) = &app.identity {
        let tag = format!("@{identity}");
        let x = (left + content_width).saturating_sub(tag.chars().count() as u16);
        queue!(out, MoveTo(x, title_y), Print(tag))?;
    }
    queue!(out, ResetColor)?;

    match app.screen {
        Screen::Leaderboard => draw_leaderboard(out, app, left, title_y + 2, height)?,
        screen => {
            if screen == Screen::Results {
                queue!(
                    out,
                    MoveTo(left, title_y + 2),
                    SetForegroundColor(Color::Green),
                    Print(format!(
                        "{:.0} wpm   {:.1}% accuracy   {:.1}s",
                        app.wpm(),
                        app.accuracy(),
                        app.elapsed().as_secs_f64()
                    )),
                )?;
                if let Some(status) = &app.status {
                    queue!(
                        out,
                        MoveTo(left, title_y + 3),
                        SetForegroundColor(Color::Yellow),
                        Print(status),
                    )?;
                }
                queue!(out, ResetColor)?;
            }
            draw_code(out, app, left, title_y + 5, height.saturating_sub(10))?;
        }
    }

    let footer_y = height.saturating_sub(2);
    queue!(out, MoveTo(left, footer_y), SetForegroundColor(Color::DarkGrey))?;
    match app.screen {
        Screen::Results => queue!(
            out,
            SetForegroundColor(Color::Green),
            SetAttribute(Attribute::Bold),
            Print("complete!"),
            SetAttribute(Attribute::Reset),
            SetForegroundColor(Color::DarkGrey),
            Print("  enter/n next   l leaderboard   ctrl-r retry   esc quit")
        )?,
        Screen::Leaderboard => queue!(out, Print("any key to go back   esc quit"))?,
        Screen::Typing => queue!(
            out,
            Print("newlines + indentation are automatic   ctrl-r restart   esc quit")
        )?,
    }
    queue!(out, ResetColor)?;
    out.flush()
}

fn draw_leaderboard<W: Write>(
    out: &mut W,
    app: &App,
    left: u16,
    top: u16,
    height: u16,
) -> io::Result<()> {
    if app.leaderboard.is_empty() {
        return queue!(
            out,
            MoveTo(left, top + 1),
            SetForegroundColor(Color::DarkGrey),
            Print("no scores yet. be the first."),
            ResetColor
        );
    }
    let rows = LEADERBOARD_ROWS.min(height.saturating_sub(top + 4) as usize);
    for (rank, entry) in app.leaderboard.iter().take(rows).enumerate() {
        let is_me = app.identity.as_deref() == Some(entry.login.as_str());
        queue!(
            out,
            MoveTo(left, top + rank as u16),
            SetForegroundColor(if is_me { Color::Cyan } else { Color::Grey }),
            Print(format!(
                "{:>3}  {:<24} {:>5.0} wpm  {:>5.1}%",
                rank + 1,
                entry.login,
                entry.wpm,
                entry.accuracy
            )),
            ResetColor
        )?;
    }
    Ok(())
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
    queue!(
        out,
        MoveTo(left, top),
        SetForegroundColor(Color::DarkGrey),
        Print(format!("{line_no:>2}  "))
    )?;

    for (index, ch) in target.iter().copied().enumerate() {
        if row >= max_rows {
            break;
        }
        if ch == '\n' {
            row += 1;
            col = 0;
            line_no += 1;
            if row < max_rows {
                queue!(
                    out,
                    MoveTo(left, top + row),
                    SetForegroundColor(Color::DarkGrey),
                    Print(format!("{line_no:>2}  "))
                )?;
            }
            continue;
        }
        queue!(out, MoveTo(left + 4 + col, top + row))?;
        if let Some(actual) = app.typed.get(index) {
            if *actual == ch {
                queue!(out, SetForegroundColor(Color::Green), Print(ch))?;
            } else {
                let shown = if *actual == '\n' { '↵' } else { *actual };
                queue!(
                    out,
                    SetForegroundColor(Color::Red),
                    SetAttribute(Attribute::Underlined),
                    Print(shown),
                    SetAttribute(Attribute::NoUnderline)
                )?;
            }
        } else if index == app.typed.len() && app.screen == Screen::Typing {
            queue!(
                out,
                SetForegroundColor(Color::Black),
                SetBackgroundColor(Color::Cyan),
                Print(ch),
                ResetColor
            )?;
        } else {
            queue!(
                out,
                SetForegroundColor(syntax_color(&target, index)),
                Print(ch)
            )?;
        }
        col += 1;
    }
    queue!(out, ResetColor)
}
