//! The terminal from C: whether there is one and whether it's Flow, raw keys
//! and the alternate screen, its size, what it can do (asked once and kept
//! in a `FlowTerminal`), sending graphics and effects where they go (down
//! Flow's pipe on Windows, else to standard output), and the next key,
//! click or resize.
//!
//! Outside a terminal these do nothing rather than reach past standard
//! input and output (crossterm would open `/dev/tty`): a program run with
//! its input or output redirected, as in a test, gets a plain terminal and
//! falls back quietly.

use std::ffi::{c_char, c_int, c_void};
use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};

use flow_sdk::crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use flow_sdk::crossterm::terminal;
use flow_sdk::{input, term, Query, Terminal};

use crate::{guard, owned_or_null, slice, status, text};

/// `FLOW_ASK_*`: which questions [`flow_detect`] asks.
pub const ASK_PICTURES: u32 = 1;
pub const ASK_MODELS: u32 = 2;
pub const ASK_CELL: u32 = 4;
pub const ASK_AREA: u32 = 8;
pub const ASK_VERSION: u32 = 16;
/// Asks nothing down Flow's pipe on Windows.
pub const ASK_NO_PIPE: u32 = 32;
pub const ASK_BACKGROUND: u32 = 64;

/// `FLOW_EVENT_*`: what [`flow_next_event`] read.
pub const EVENT_KEY: i32 = 1;
pub const EVENT_CLICK: i32 = 2;
pub const EVENT_RESIZE: i32 = 3;
pub const EVENT_OTHER: i32 = 4;

/// `FLOW_KEY_*` for keys without a character: past Unicode's last code
/// point, so they can't be mistaken for one. Enter, Tab, Backspace and
/// Escape are the control characters a terminal sends for them.
pub const KEY_ENTER: u32 = 13;
pub const KEY_TAB: u32 = 9;
pub const KEY_BACKSPACE: u32 = 127;
pub const KEY_ESCAPE: u32 = 27;
pub const KEY_OTHER: u32 = 0x11_0000;
pub const KEY_LEFT: u32 = 0x11_0001;
pub const KEY_RIGHT: u32 = 0x11_0002;
pub const KEY_UP: u32 = 0x11_0003;
pub const KEY_DOWN: u32 = 0x11_0004;
pub const KEY_HOME: u32 = 0x11_0005;
pub const KEY_END: u32 = 0x11_0006;
pub const KEY_PAGE_UP: u32 = 0x11_0007;
pub const KEY_PAGE_DOWN: u32 = 0x11_0008;
pub const KEY_DELETE: u32 = 0x11_0009;
pub const KEY_INSERT: u32 = 0x11_000A;
pub const KEY_BACKTAB: u32 = 0x11_000B;
/// F1; F2 to F24 follow on.
pub const KEY_F1: u32 = 0x11_0101;

/// `FLOW_MOD_*`, held with a key.
pub const MOD_SHIFT: u32 = 1;
pub const MOD_CTRL: u32 = 2;
pub const MOD_ALT: u32 = 4;

/// The terminal's size: cells, and a cell's pixels on the screen where the
/// terminal tells (0 where it doesn't).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FlowSize {
    pub cols: u16,
    pub rows: u16,
    pub cell_width: f32,
    pub cell_height: f32,
}

/// What the terminal said it can do ([`flow_sdk::Caps`]), sizes 0 where
/// it didn't say.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FlowCaps {
    pub pictures: bool,
    pub models: bool,
    pub flow: bool,
    pub bare: bool,
    pub piped: bool,
    pub cell_width: u16,
    pub cell_height: u16,
    pub area_width: u32,
    pub area_height: u32,
}

/// A key, a click or a resize.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FlowEvent {
    /// `FLOW_EVENT_*`.
    pub kind: i32,
    /// A key's character as a code point, or one of `FLOW_KEY_*`.
    pub key: u32,
    /// `FLOW_MOD_*` held with a key.
    pub mods: u32,
    /// Where a click went down, counting cells from 0; or the new size.
    pub col: u16,
    pub row: u16,
    /// A click's button: 0 left, 1 right, 2 middle.
    pub button: u32,
}

/// The terminal as asked: what it can do, and where pictures go.
pub struct FlowTerminal(pub(crate) Terminal);

/// Whether standard input and output are both a terminal: the questions
/// go out one way and the answers come back the other.
fn in_terminal() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

#[no_mangle]
pub extern "C" fn flow_in_flow() -> bool {
    guard(false, flow_sdk::in_flow)
}

#[no_mangle]
pub extern "C" fn flow_is_terminal() -> bool {
    guard(false, in_terminal)
}

/// Raw keys on or off, keeping the screen: for asking the terminal below
/// the prompt. -1 outside a terminal.
#[no_mangle]
pub extern "C" fn flow_raw_mode(on: bool) -> c_int {
    guard(-1, || {
        if !io::stdin().is_terminal() {
            return -1;
        }
        status(if on { terminal::enable_raw_mode() } else { terminal::disable_raw_mode() })
    })
}

#[no_mangle]
pub extern "C" fn flow_term_enter(mouse: bool) -> c_int {
    guard(-1, || {
        if !in_terminal() {
            return -1;
        }
        let result = term::enter(mouse);
        if result.is_err() {
            term::leave();
        }
        status(result)
    })
}

#[no_mangle]
pub extern "C" fn flow_term_leave() {
    guard((), || {
        if in_terminal() {
            term::leave();
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn flow_term_size(out: *mut FlowSize) -> c_int {
    guard(-1, || {
        let Some(out) = out.as_mut() else { return -1 };
        if !in_terminal() {
            return -1;
        }
        let Ok(size) = term::size() else { return -1 };
        let (cell_width, cell_height) = size.cell.unwrap_or((0.0, 0.0));
        *out = FlowSize { cols: size.cols, rows: size.rows, cell_width, cell_height };
        0
    })
}

/// The SDK's query for `ask`'s flags, waiting `wait_ms` (the SDK's second
/// if 0) for a terminal that answers nothing.
fn query(ask: u32, wait_ms: u32) -> Query {
    let mut q = Query::new();
    if ask & ASK_PICTURES != 0 {
        q = q.pictures();
    }
    if ask & ASK_MODELS != 0 {
        q = q.models();
    }
    if ask & ASK_CELL != 0 {
        q = q.cell();
    }
    if ask & ASK_AREA != 0 {
        q = q.area();
    }
    if ask & ASK_VERSION != 0 {
        q = q.version();
    }
    if ask & ASK_BACKGROUND != 0 {
        q = q.background();
    }
    if ask & ASK_NO_PIPE != 0 {
        q = q.without_pipe();
    }
    if wait_ms > 0 {
        q = q.wait(Duration::from_millis(u64::from(wait_ms)));
    }
    q
}

/// Asks the terminal once. On Unix the questions go only to a terminal:
/// with input or output redirected, reading the answers would take the
/// program's own input, so it's a plain terminal then, known as Flow only
/// by name. On Windows they go down Flow's pipe, which doesn't touch either.
fn ask(query: &Query) -> Terminal {
    if cfg!(unix) && !in_terminal() {
        let mut plain = Terminal::plain();
        plain.caps.flow = flow_sdk::in_flow();
        return plain;
    }
    query.ask()
}

#[no_mangle]
pub extern "C" fn flow_detect(ask_flags: u32, wait_ms: u32) -> *mut FlowTerminal {
    guard(std::ptr::null_mut(), || Box::into_raw(Box::new(FlowTerminal(ask(&query(ask_flags, wait_ms))))))
}

#[no_mangle]
pub extern "C" fn flow_terminal_plain() -> *mut FlowTerminal {
    guard(std::ptr::null_mut(), || Box::into_raw(Box::new(FlowTerminal(Terminal::plain()))))
}

#[no_mangle]
pub unsafe extern "C" fn flow_terminal_free(terminal: *mut FlowTerminal) {
    if !terminal.is_null() {
        guard((), || drop(Box::from_raw(terminal)));
    }
}

#[no_mangle]
pub unsafe extern "C" fn flow_terminal_caps(terminal: *const FlowTerminal, out: *mut FlowCaps) -> c_int {
    guard(-1, || {
        let (Some(terminal), Some(out)) = (terminal.as_ref(), out.as_mut()) else { return -1 };
        let caps = &terminal.0.caps;
        let (cell_width, cell_height) = caps.cell.unwrap_or((0, 0));
        let (area_width, area_height) = caps.area.unwrap_or((0, 0));
        *out = FlowCaps {
            pictures: caps.pictures,
            models: caps.models,
            flow: caps.flow,
            bare: caps.bare,
            piped: terminal.0.piped(),
            cell_width,
            cell_height,
            area_width,
            area_height,
        };
        0
    })
}

/// XTVERSION's answer (`Flow 0.1.24`, say), or NULL if it gave none.
#[no_mangle]
pub unsafe extern "C" fn flow_terminal_version(terminal: *const FlowTerminal) -> *mut c_char {
    owned_or_null(|| terminal.as_ref()?.0.caps.version.clone())
}

/// The background OSC 11 gave, as 0xRRGGBB in `rgb`; -1 if it gave none.
#[no_mangle]
pub unsafe extern "C" fn flow_terminal_background(terminal: *const FlowTerminal, rgb: *mut u32) -> c_int {
    guard(-1, || {
        let (Some(terminal), Some(rgb)) = (terminal.as_ref(), rgb.as_mut()) else { return -1 };
        let Some([r, g, b]) = terminal.0.caps.background else { return -1 };
        *rgb = u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b);
        0
    })
}

/// Whether 0xRRGGBB is light, as the SDK judges a background
/// ([`flow_sdk::detect::is_light`]).
#[no_mangle]
pub extern "C" fn flow_is_light(rgb: u32) -> bool {
    guard(false, || flow_sdk::detect::is_light([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]))
}

#[no_mangle]
pub unsafe extern "C" fn flow_terminal_cell_size(terminal: *mut FlowTerminal, width: *mut f32, height: *mut f32) -> c_int {
    guard(-1, || {
        let (Some(terminal), Some(width), Some(height)) = (terminal.as_mut(), width.as_mut(), height.as_mut()) else { return -1 };
        if cfg!(unix) && !in_terminal() {
            return -1;
        }
        let Some((w, h)) = terminal.0.cell_size() else { return -1 };
        (*width, *height) = (w, h);
        0
    })
}

#[no_mangle]
pub unsafe extern "C" fn flow_terminal_send(terminal: *mut FlowTerminal, bytes: *const c_void, len: usize) -> c_int {
    guard(-1, || {
        let (Some(terminal), Some(bytes)) = (terminal.as_mut(), slice(bytes.cast::<u8>(), len)) else { return -1 };
        status(terminal.0.send(bytes))
    })
}

#[no_mangle]
pub unsafe extern "C" fn flow_terminal_send_effects(terminal: *mut FlowTerminal, bytes: *const c_void, len: usize) -> c_int {
    guard(-1, || {
        let (Some(terminal), Some(bytes)) = (terminal.as_mut(), slice(bytes.cast::<u8>(), len)) else { return -1 };
        status(terminal.0.send_effects(bytes))
    })
}

fn write_out(bytes: &[u8]) -> io::Result<()> {
    let mut out = io::stdout().lock();
    out.write_all(bytes)?;
    out.flush()
}

/// Writes a string to standard output and flushes it.
#[no_mangle]
pub unsafe extern "C" fn flow_write(escape: *const c_char) -> c_int {
    guard(-1, || {
        let Some(escape) = text(escape) else { return -1 };
        status(write_out(escape.as_bytes()))
    })
}

/// [`flow_write`], then [`crate::flow_free`]: for a string just made.
#[no_mangle]
pub unsafe extern "C" fn flow_write_free(escape: *mut c_char) -> c_int {
    if escape.is_null() {
        return -1;
    }
    let result = flow_write(escape);
    crate::flow_free(escape.cast());
    result
}

/// The next key, click or resize, waiting `timeout_ms` at most: 1 when
/// `out` holds one, 0 when the time ran out, -1 outside a terminal or when
/// the keys can't be read.
#[no_mangle]
pub unsafe extern "C" fn flow_next_event(timeout_ms: u32, out: *mut FlowEvent) -> c_int {
    guard(-1, || {
        let Some(out) = out.as_mut() else { return -1 };
        if !io::stdin().is_terminal() {
            return -1;
        }
        match input::next_before(Instant::now() + Duration::from_millis(u64::from(timeout_ms))) {
            Ok(Some(event)) => {
                *out = event_for(&event);
                1
            }
            Ok(None) => 0,
            Err(_) => -1,
        }
    })
}

/// A crossterm event as C reads it.
pub(crate) fn event_for(event: &Event) -> FlowEvent {
    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release => {
            let code = match key.code {
                KeyCode::Char(c) => u32::from(c),
                KeyCode::Enter => KEY_ENTER,
                KeyCode::Tab => KEY_TAB,
                KeyCode::Backspace => KEY_BACKSPACE,
                KeyCode::Esc => KEY_ESCAPE,
                KeyCode::Left => KEY_LEFT,
                KeyCode::Right => KEY_RIGHT,
                KeyCode::Up => KEY_UP,
                KeyCode::Down => KEY_DOWN,
                KeyCode::Home => KEY_HOME,
                KeyCode::End => KEY_END,
                KeyCode::PageUp => KEY_PAGE_UP,
                KeyCode::PageDown => KEY_PAGE_DOWN,
                KeyCode::Delete => KEY_DELETE,
                KeyCode::Insert => KEY_INSERT,
                KeyCode::BackTab => KEY_BACKTAB,
                KeyCode::F(n @ 1..=24) => KEY_F1 + u32::from(n) - 1,
                _ => KEY_OTHER,
            };
            let held = [(KeyModifiers::SHIFT, MOD_SHIFT), (KeyModifiers::CONTROL, MOD_CTRL), (KeyModifiers::ALT, MOD_ALT)];
            let mods = held.iter().filter(|(m, _)| key.modifiers.contains(*m)).fold(0, |all, (_, bit)| all | bit);
            FlowEvent { kind: EVENT_KEY, key: code, mods, ..FlowEvent::default() }
        }
        Event::Mouse(m) => match m.kind {
            MouseEventKind::Down(button) => {
                let button = match button {
                    MouseButton::Left => 0,
                    MouseButton::Right => 1,
                    MouseButton::Middle => 2,
                };
                FlowEvent { kind: EVENT_CLICK, col: m.column, row: m.row, button, ..FlowEvent::default() }
            }
            _ => FlowEvent { kind: EVENT_OTHER, col: m.column, row: m.row, ..FlowEvent::default() },
        },
        Event::Resize(cols, rows) => FlowEvent { kind: EVENT_RESIZE, col: *cols, row: *rows, ..FlowEvent::default() },
        _ => FlowEvent { kind: EVENT_OTHER, ..FlowEvent::default() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flow_sdk::crossterm::event::{KeyEvent, MouseEvent};

    #[test]
    fn the_flags_ask_what_they_name_and_da1_last() {
        let q = query(ASK_MODELS | ASK_CELL, 0);
        assert_eq!(q, Query::new().models().cell());
        assert_eq!(query(ASK_PICTURES | ASK_AREA | ASK_VERSION | ASK_NO_PIPE, 250), Query::new().pictures().area().version().without_pipe().wait(Duration::from_millis(250)));
        assert_eq!(query(ASK_BACKGROUND, 0), Query::new().background());
    }

    #[test]
    fn the_background_comes_as_c_has_colors_and_is_judged_as_the_sdk_does() {
        let mut terminal = Terminal::plain();
        terminal.caps.background = Some([0xd6, 0xd2, 0xc4]);
        let terminal = FlowTerminal(terminal);
        let mut rgb = 0;
        unsafe {
            assert_eq!(flow_terminal_background(&terminal, &mut rgb), 0);
            assert_eq!(flow_terminal_background(&terminal, std::ptr::null_mut()), -1);
            let plain = flow_terminal_plain();
            assert_eq!(flow_terminal_background(plain, &mut rgb), -1, "it didn't say");
            flow_terminal_free(plain);
        }
        assert_eq!(rgb, 0xd6d2c4);
        assert!(flow_is_light(rgb) && flow_is_light(0xffffff) && !flow_is_light(0x080c16));
    }

    #[test]
    fn a_plain_terminal_says_it_can_do_nothing() {
        let terminal = flow_terminal_plain();
        let mut caps = FlowCaps { pictures: true, ..FlowCaps::default() };
        unsafe {
            assert_eq!(flow_terminal_caps(terminal, &mut caps), 0);
            assert!(flow_terminal_version(terminal).is_null());
            assert_eq!(flow_terminal_caps(terminal, std::ptr::null_mut()), -1);
            assert_eq!(flow_terminal_caps(std::ptr::null(), &mut caps), -1);
            assert_eq!(flow_terminal_send(std::ptr::null_mut(), b"x".as_ptr().cast(), 1), -1);
            assert_eq!(flow_terminal_send(terminal, std::ptr::null(), 1), -1, "NULL with a length");
            assert_eq!(flow_terminal_send(terminal, std::ptr::null(), 0), 0, "nothing to send");
            flow_terminal_free(terminal);
            flow_terminal_free(std::ptr::null_mut());
        }
        assert_eq!(caps, FlowCaps::default());
    }

    #[test]
    fn keys_clicks_and_resizes_read_as_c_has_them() {
        let key = |code, modifiers| event_for(&Event::Key(KeyEvent::new(code, modifiers)));
        assert_eq!(key(KeyCode::Char('q'), KeyModifiers::NONE), FlowEvent { kind: EVENT_KEY, key: 'q' as u32, ..FlowEvent::default() });
        assert_eq!(key(KeyCode::Char('c'), KeyModifiers::CONTROL).mods, MOD_CTRL);
        assert_eq!(key(KeyCode::Up, KeyModifiers::SHIFT | KeyModifiers::ALT), FlowEvent { kind: EVENT_KEY, key: KEY_UP, mods: MOD_SHIFT | MOD_ALT, ..FlowEvent::default() });
        assert_eq!(key(KeyCode::F(12), KeyModifiers::NONE).key, KEY_F1 + 11);
        assert_eq!(key(KeyCode::Enter, KeyModifiers::NONE).key, KEY_ENTER);
        let mouse = |kind| event_for(&Event::Mouse(MouseEvent { kind, column: 7, row: 3, modifiers: KeyModifiers::NONE }));
        assert_eq!(mouse(MouseEventKind::Down(MouseButton::Right)), FlowEvent { kind: EVENT_CLICK, col: 7, row: 3, button: 1, ..FlowEvent::default() });
        assert_eq!(mouse(MouseEventKind::Up(MouseButton::Left)).kind, EVENT_OTHER);
        assert_eq!(event_for(&Event::Resize(120, 40)), FlowEvent { kind: EVENT_RESIZE, col: 120, row: 40, ..FlowEvent::default() });
    }

    #[test]
    fn outside_a_terminal_nothing_is_asked_or_read() {
        if in_terminal() {
            return;
        }
        let terminal = flow_detect(ASK_PICTURES | ASK_MODELS | ASK_VERSION, 10);
        let mut caps = FlowCaps::default();
        let mut size = FlowSize::default();
        let mut event = FlowEvent::default();
        unsafe {
            assert_eq!(flow_terminal_caps(terminal, &mut caps), 0);
            assert!(!caps.pictures && !caps.models && caps.flow == flow_sdk::in_flow());
            assert_eq!(flow_term_size(&mut size), -1);
            flow_terminal_free(terminal);
        }
        if !io::stdin().is_terminal() {
            assert_eq!(unsafe { flow_next_event(0, &mut event) }, -1);
            assert_eq!(flow_raw_mode(true), -1);
        }
        assert_eq!(flow_term_enter(false), -1);
    }
}
