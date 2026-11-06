//! The terminal itself: raw keys on the alternate screen, with the mouse if
//! the program wants it, handed back as it was, and its size in cells and in
//! pixels.

use std::io::{self, Write};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::{cursor, execute, terminal};

/// The terminal's size: cells, and a cell's size on the screen in pixels,
/// fractions and all, where it tells (the pixels the pty was given; Flow
/// gives them on macOS and Linux, ConPTY on Windows gives none).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    /// Columns.
    pub cols: u16,
    /// Rows.
    pub rows: u16,
    /// A cell's width and height in pixels, where the terminal tells.
    pub cell: Option<(f32, f32)>,
}

/// The terminal's size now.
pub fn size() -> io::Result<Size> {
    let (cols, rows) = terminal::size()?;
    let cell = terminal::window_size()
        .ok()
        .filter(|w| w.width > 0 && w.height > 0 && w.columns > 0 && w.rows > 0)
        .map(|w| (w.width as f32 / w.columns as f32, w.height as f32 / w.rows as f32));
    Ok(Size { cols, rows, cell })
}

/// Raw keys, the alternate screen, no cursor or line wrapping, a clear
/// screen, and with `mouse` the mouse's clicks, drags and moves (as SGR
/// reports, which Flow sends). [`leave`] undoes it all.
pub fn enter(mouse: bool) -> io::Result<()> {
    terminal::enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(out, terminal::EnterAlternateScreen, cursor::Hide)?;
    if mouse {
        execute!(out, EnableMouseCapture)?;
    }
    out.write_all(b"\x1b[?7l\x1b[2J")?;
    out.flush()
}

/// Hands the terminal back as it was: colors, line wrapping, the mouse
/// (turned off whether or not it was on), the cursor, the main screen and
/// cooked keys. Errors are passed over, since it's what runs on the way
/// out, each on its own: on Windows crossterm turns the mouse off through
/// the console, which fails if it was never on, and that mustn't keep the
/// main screen from coming back.
pub fn leave() {
    let mut out = io::stdout();
    let _ = out.write_all(b"\x1b[0m\x1b[?7h");
    let _ = execute!(out, DisableMouseCapture);
    let _ = execute!(out, cursor::Show);
    let _ = execute!(out, terminal::LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
}

/// Makes a panic [`leave`] first, so it doesn't leave the terminal raw
/// with its message lost on the alternate screen.
pub fn leave_on_panic() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        leave();
        hook(info);
    }));
}
