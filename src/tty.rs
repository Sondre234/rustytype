//! The terminal in raw mode, driven through libc, which std already links on
//! Unix.

use std::collections::VecDeque;
use std::ffi::{c_int, c_short, c_ulong};
use std::fs::File;
use std::io::{self, ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::panic;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::ansi::{ENTER_ALTERNATE_SCREEN, HIDE_CURSOR, LEAVE_ALTERNATE_SCREEN, RESET, SHOW_CURSOR};
use crate::app::Key;

/// `struct termios` is only ever filled in and read by libc, so it can stay
/// opaque instead of spelling out a layout that differs per platform. This is
/// larger than any of them.
#[repr(C, align(8))]
#[derive(Clone, Copy)]
struct Termios([u8; 256]);

#[repr(C)]
struct PollFd {
    fd: c_int,
    events: c_short,
    revents: c_short,
}

#[repr(C)]
#[derive(Default)]
struct WinSize {
    rows: u16,
    cols: u16,
    x_pixels: u16,
    y_pixels: u16,
}

const TCSANOW: c_int = 0;
const POLLIN: c_short = 1;
#[cfg(target_os = "linux")]
const TIOCGWINSZ: c_ulong = 0x5413;
#[cfg(target_os = "macos")]
const TIOCGWINSZ: c_ulong = 0x4008_7468;
#[cfg(target_os = "linux")]
type NfdsT = c_ulong;
#[cfg(target_os = "macos")]
type NfdsT = std::ffi::c_uint;

unsafe extern "C" {
    fn tcgetattr(fd: c_int, termios: *mut Termios) -> c_int;
    fn tcsetattr(fd: c_int, when: c_int, termios: *const Termios) -> c_int;
    fn cfmakeraw(termios: *mut Termios);
    fn poll(fds: *mut PollFd, count: NfdsT, timeout_ms: c_int) -> c_int;
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
}

/// Whether the terminal still needs restoring. On a panic both the hook and
/// `Drop` restore it, and leaving the alternate screen a second time moves the
/// cursor back up, where the shell prompt then covers the panic message.
static RAW: AtomicBool = AtomicBool::new(false);

fn check(result: c_int) -> io::Result<c_int> {
    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result)
    }
}

pub struct Terminal {
    tty: File,
    saved: Termios,
    keys: VecDeque<Key>,
}

impl Terminal {
    /// Raw mode on the alternate screen, until this is dropped or the program
    /// panics.
    pub fn enter() -> io::Result<Self> {
        let tty = File::options().read(true).write(true).open("/dev/tty")?;
        let mut saved = Termios([0; 256]);
        // SAFETY: `Termios` is big enough and aligned for any `struct termios`.
        check(unsafe { tcgetattr(tty.as_raw_fd(), &mut saved) })?;
        let mut raw = saved;
        // SAFETY: `raw` holds the valid settings `tcgetattr` just filled in.
        unsafe { cfmakeraw(&mut raw) };
        // SAFETY: as above.
        check(unsafe { tcsetattr(tty.as_raw_fd(), TCSANOW, &raw) })?;
        RAW.store(true, Ordering::SeqCst);
        // From here on, dropping `terminal` puts the settings back.
        let terminal = Self { tty, saved, keys: VecDeque::new() };

        // Restore before the panic message prints, or it goes to the alternate
        // screen and vanishes with it.
        let tty = terminal.tty.try_clone()?;
        let previous_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            restore(&tty, &saved);
            previous_hook(info);
        }));

        terminal.show(format!("{ENTER_ALTERNATE_SCREEN}{HIDE_CURSOR}").as_bytes())?;
        Ok(terminal)
    }

    /// Columns and rows.
    pub fn size(&self) -> io::Result<(u16, u16)> {
        let mut size = WinSize::default();
        // SAFETY: TIOCGWINSZ writes one `struct winsize`, which `WinSize` mirrors.
        check(unsafe { ioctl(self.tty.as_raw_fd(), TIOCGWINSZ, &mut size as *mut WinSize) })?;
        Ok((size.cols, size.rows))
    }

    pub fn show(&self, frame: &[u8]) -> io::Result<()> {
        (&self.tty).write_all(frame)
    }

    /// The next key, or `None` if `timeout` passes without one.
    pub fn read_key(&mut self, timeout: Duration) -> io::Result<Option<Key>> {
        if self.keys.is_empty() {
            let mut fd = PollFd { fd: self.tty.as_raw_fd(), events: POLLIN, revents: 0 };
            // SAFETY: `fd` is one valid `struct pollfd`.
            match check(unsafe { poll(&mut fd, 1, timeout.as_millis() as c_int) }) {
                Ok(0) => {}
                Ok(_) => {
                    let mut bytes = [0; 1024];
                    let read = self.tty.read(&mut bytes)?;
                    parse_keys(&String::from_utf8_lossy(&bytes[..read]), &mut self.keys);
                }
                // A signal cut the wait short; the caller just asks again.
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        Ok(self.keys.pop_front())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        restore(&self.tty, &self.saved);
    }
}

/// Best effort: this runs on the way out, where a failure has nowhere to go.
fn restore(mut tty: &File, saved: &Termios) {
    if !RAW.swap(false, Ordering::SeqCst) {
        return;
    }
    let _ = write!(tty, "{RESET}{SHOW_CURSOR}{LEAVE_ALTERNATE_SCREEN}");
    // SAFETY: `saved` holds the settings `tcgetattr` filled in.
    unsafe { tcsetattr(tty.as_raw_fd(), TCSANOW, saved) };
}

/// Decode the bytes a terminal sends in raw mode. Ctrl-<letter> arrives as
/// the letter's position in the alphabet.
fn parse_keys(input: &str, keys: &mut VecDeque<Key>) {
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        let key = match ch {
            // Esc arrives alone; anything right behind it is an escape
            // sequence (arrow keys, Alt+key), which the game has no use for.
            '\x1b' => match chars.next() {
                None => Key::Quit,
                Some('[' | 'O') => {
                    // Parameters run up to a final byte in `@`..=`~`.
                    chars.find(|c| ('@'..='~').contains(c));
                    continue;
                }
                Some(_) => continue,
            },
            '\x03' => Key::Quit,
            '\x12' => Key::Restart,
            '\x7f' | '\x08' => Key::Backspace,
            '\r' | '\n' => Key::Enter,
            ch if ch.is_control() => continue,
            ch => Key::Char(ch),
        };
        keys.push_back(key);
    }
}
