//! The few ANSI escape sequences the game needs.

use std::fmt;

pub const CLEAR: &str = "\x1b[2J";
pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const UNDERLINE: &str = "\x1b[4m";
pub const ENTER_ALTERNATE_SCREEN: &str = "\x1b[?1049h";
pub const LEAVE_ALTERNATE_SCREEN: &str = "\x1b[?1049l";
pub const HIDE_CURSOR: &str = "\x1b[?25l";
pub const SHOW_CURSOR: &str = "\x1b[?25h";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Color {
    Black,
    Grey,
    DarkGrey,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
}

impl Color {
    /// Index in the terminal's palette. The bright variants (9-14) read better
    /// than the plain ones on dark themes.
    fn index(self) -> u8 {
        match self {
            Color::Black => 0,
            Color::Grey => 7,
            Color::DarkGrey => 8,
            Color::Red => 9,
            Color::Green => 10,
            Color::Yellow => 11,
            Color::Blue => 12,
            Color::Magenta => 13,
            Color::Cyan => 14,
        }
    }
}

pub struct Fg(pub Color);

impl fmt::Display for Fg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\x1b[38;5;{}m", self.0.index())
    }
}

pub struct Bg(pub Color);

impl fmt::Display for Bg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\x1b[48;5;{}m", self.0.index())
    }
}

/// Zero-based column and row; the terminal counts from one.
pub struct MoveTo(pub u16, pub u16);

impl fmt::Display for MoveTo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\x1b[{};{}H", self.1 + 1, self.0 + 1)
    }
}
