//! Keys and the mouse, as Flow reports them, read with crossterm (which the
//! crate re-exports as `flow_sdk::crossterm`, so a program reads with the
//! same version). Flow reports the mouse over the terminal to a program
//! that asks for it (`?1000h` clicks, `?1002h` drags, `?1003h` every move)
//! as SGR reports (`?1006h`), which crossterm reads; holding Shift keeps the
//! mouse Flow's own, for selecting and its menu. [`crate::term::enter`]
//! asks for all of it.

use std::io;
use std::time::Instant;

pub use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

/// How much of the mouse to report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tracking {
    /// Buttons going down and up, and the wheel.
    Clicks,
    /// Those, and moves while a button is held.
    Drags,
    /// Every move.
    Moves,
}

/// The escapes that turn the mouse's reports on, as SGR reports.
pub fn mouse_on(tracking: Tracking) -> &'static str {
    match tracking {
        Tracking::Clicks => "\x1b[?1000h\x1b[?1006h",
        Tracking::Drags => "\x1b[?1000h\x1b[?1002h\x1b[?1006h",
        Tracking::Moves => "\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h",
    }
}

/// The escapes that turn them off again, whichever were on.
pub const MOUSE_OFF: &str = "\x1b[?1006l\x1b[?1003l\x1b[?1002l\x1b[?1000l";

/// The next event worth handling, waiting until `deadline` at most: keys as
/// they go down or repeat (not as they come up, which Windows reports too),
/// the mouse, pastes, focus and resizes. None once the deadline passes and
/// nothing is waiting: one that has passed still takes what's there, or a
/// program whose frames take longer than it allows them (a big scene down
/// Flow's pipe on Windows) would never hear a key.
pub fn next_before(deadline: Instant) -> io::Result<Option<Event>> {
    loop {
        let wait = deadline.saturating_duration_since(Instant::now());
        if !crossterm::event::poll(wait)? {
            return Ok(None);
        }
        match crossterm::event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Release => {}
            event => return Ok(Some(event)),
        }
    }
}

/// q, Esc or Ctrl+C: what quits a program that takes the whole screen.
pub fn is_quit(key: &KeyEvent) -> bool {
    let ctrl_c = key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
    ctrl_c || matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
}

/// Where a left click went down, in cells from the top-left from 0.
pub fn left_click(event: &Event) -> Option<(u16, u16)> {
    match event {
        Event::Mouse(m) if m.kind == MouseEventKind::Down(MouseButton::Left) => Some((m.column, m.row)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q_esc_and_ctrl_c_quit_and_a_c_alone_doesnt() {
        let key = |code, modifiers| KeyEvent::new(code, modifiers);
        assert!(is_quit(&key(KeyCode::Char('q'), KeyModifiers::NONE)));
        assert!(is_quit(&key(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(is_quit(&key(KeyCode::Char('c'), KeyModifiers::CONTROL)));
        assert!(!is_quit(&key(KeyCode::Char('c'), KeyModifiers::NONE)));
    }

    #[test]
    fn a_left_click_says_where_and_other_buttons_dont() {
        let mouse = |kind| Event::Mouse(MouseEvent { kind, column: 4, row: 2, modifiers: KeyModifiers::NONE });
        assert_eq!(left_click(&mouse(MouseEventKind::Down(MouseButton::Left))), Some((4, 2)));
        assert_eq!(left_click(&mouse(MouseEventKind::Down(MouseButton::Right))), None);
        assert_eq!(left_click(&mouse(MouseEventKind::Up(MouseButton::Left))), None);
        assert!(mouse_on(Tracking::Moves).ends_with("\x1b[?1006h"));
    }
}
