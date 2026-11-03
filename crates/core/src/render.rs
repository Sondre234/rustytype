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

/// Draw one full frame for a terminal of the given size.
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
    queue!(out, Print(format!("  / {}", app.title())), ResetColor)?;

    if app.screen == Screen::Results {
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
            ResetColor
        )?;
    }
    draw_code(out, app, left, title_y + 5, height.saturating_sub(10))?;

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
            Print("  enter/n next   ctrl-r retry   esc quit")
        )?,
        Screen::Typing => queue!(
            out,
            Print(if app.autoclose() {
                "newlines, indents + closers are automatic   ctrl-r restart   esc quit"
            } else {
                "newlines + indentation are automatic   ctrl-r restart   esc quit"
            })
        )?,
    }
    queue!(out, ResetColor)?;
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
        } else if app.auto_closed(&target, index) {
            // Already in place ahead of the cursor, as in an editor.
            queue!(out, SetForegroundColor(Color::Green), Print(ch))?;
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
