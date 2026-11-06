//! Screens from C ([`flow_sdk::screen`]): a screen of cells drawn each
//! frame, letters in their colors with pieces of pictures among them, sent
//! as its difference from the one shown before it: only the cells that
//! changed. A `FlowScreen` holds both, the one being drawn and the one
//! shown last, so a program draws into it and asks for what to send.

use std::ffi::{c_char, c_int};

use flow_sdk::screen::{Rgb, Screen};

use crate::{guard, owned, owned_or_null, text};

/// A screen being drawn and the one shown last: `flow_screen_new` to
/// `flow_screen_free`.
pub struct FlowScreen {
    now: Screen,
    shown: Option<Screen>,
}

/// 0xRRGGBB as a color.
fn rgb(hex: u32) -> Rgb {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8]
}

/// A background: 0xRRGGBB, or the terminal's own for any negative number.
fn background(bg: i32) -> Option<Rgb> {
    u32::try_from(bg).ok().map(rgb)
}

/// A blank screen `cols` × `rows` cells, nothing shown yet; NULL for none
/// of either.
#[no_mangle]
pub extern "C" fn flow_screen_new(cols: u16, rows: u16) -> *mut FlowScreen {
    guard(std::ptr::null_mut(), || {
        if cols == 0 || rows == 0 {
            return std::ptr::null_mut();
        }
        Box::into_raw(Box::new(FlowScreen { now: Screen::new(cols, rows), shown: None }))
    })
}

#[no_mangle]
pub unsafe extern "C" fn flow_screen_free(screen: *mut FlowScreen) {
    if !screen.is_null() {
        guard((), || drop(Box::from_raw(screen)));
    }
}

/// Writes `text` from cell (`col`, `row`), a cell a character, in `fg`
/// (0xRRGGBB) on `bg` (0xRRGGBB, or negative for the terminal's own),
/// bold or not. What falls off the screen isn't written.
#[no_mangle]
pub unsafe extern "C" fn flow_screen_text(screen: *mut FlowScreen, col: i32, row: i32, text: *const c_char, fg: u32, bg: i32, bold: bool) -> c_int {
    guard(-1, || {
        let (Some(screen), Some(text)) = (screen.as_mut(), self::text(text)) else { return -1 };
        screen.now.styled(col, row, text, rgb(fg), background(bg), bold);
        0
    })
}

/// Picture `id` over `cols` × `rows` cells from (`col`, `row`), each on
/// the background of the cell it covers: send it first, at
/// FLOW_PLACEMENT, over as many cells.
#[no_mangle]
pub unsafe extern "C" fn flow_screen_image(screen: *mut FlowScreen, col: i32, row: i32, cols: u16, rows: u16, id: u32) -> c_int {
    guard(-1, || {
        let Some(screen) = screen.as_mut() else { return -1 };
        screen.now.image(col, row, cols, rows, id);
        0
    })
}

/// Blanks `cols` × `rows` cells from (`col`, `row`), on `bg` (0xRRGGBB, or
/// negative for the terminal's own).
#[no_mangle]
pub unsafe extern "C" fn flow_screen_clear(screen: *mut FlowScreen, col: i32, row: i32, cols: i32, rows: i32, bg: i32) -> c_int {
    guard(-1, || {
        let Some(screen) = screen.as_mut() else { return -1 };
        screen.now.clear(col, row, cols, rows, background(bg));
        0
    })
}

/// Lays `bg` (0xRRGGBB) under every cell of the `cols` × `rows` from
/// (`col`, `row`) that has no background of its own, so none is left to
/// the terminal's.
#[no_mangle]
pub unsafe extern "C" fn flow_screen_ground(screen: *mut FlowScreen, col: i32, row: i32, cols: i32, rows: i32, bg: u32) -> c_int {
    guard(-1, || {
        let Some(screen) = screen.as_mut() else { return -1 };
        screen.now.ground(col, row, cols, rows, rgb(bg));
        0
    })
}

/// Row `row`'s letters, a picture's cells as spaces; NULL off the screen.
#[no_mangle]
pub unsafe extern "C" fn flow_screen_row(screen: *const FlowScreen, row: u16) -> *mut c_char {
    owned_or_null(|| {
        let screen = screen.as_ref()?;
        (row < screen.now.h).then(|| screen.now.row(row))
    })
}

/// The escapes that bring the terminal from the screen shown last (or a
/// cleared one, the first time and after `flow_screen_forget`) to this
/// one, picture cells bare if `bare` (FlowCaps.bare); this one is shown
/// from then on, and stays to draw over.
#[no_mangle]
pub unsafe extern "C" fn flow_screen_show(screen: *mut FlowScreen, bare: bool) -> *mut c_char {
    guard(std::ptr::null_mut(), || {
        let Some(screen) = screen.as_mut() else { return std::ptr::null_mut() };
        let changes = screen.now.diff(screen.shown.as_ref(), bare);
        screen.shown = Some(screen.now.clone());
        owned(changes)
    })
}

/// Has the next `flow_screen_show` draw everything, as after the terminal
/// was cleared or another program drew on it.
#[no_mangle]
pub unsafe extern "C" fn flow_screen_forget(screen: *mut FlowScreen) -> c_int {
    guard(-1, || {
        let Some(screen) = screen.as_mut() else { return -1 };
        screen.shown = None;
        0
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flow_sdk::screen::Cell;

    /// A string the library handed out, read and freed.
    fn took(s: *mut c_char) -> String {
        assert!(!s.is_null());
        // SAFETY: handed out by this crate, NUL-terminated, freed once.
        unsafe {
            let text = std::ffi::CStr::from_ptr(s).to_str().expect("UTF-8").to_string();
            crate::flow_free(s.cast());
            text
        }
    }

    #[test]
    fn a_screen_drawn_from_c_sends_what_the_sdks_does_and_then_only_its_changes() {
        let mut rust = Screen::new(12, 3);
        rust.styled(1, 0, "hp ·", [255, 0, 0], Some([4, 5, 6]), true);
        rust.image(6, 1, 2, 2, 0x10_0001);
        rust.clear(0, 2, 3, 1, Some([9, 9, 9]));
        let screen = flow_screen_new(12, 3);
        unsafe {
            assert_eq!(flow_screen_text(screen, 1, 0, c"hp ·".as_ptr(), 0xff0000, 0x040506, true), 0);
            assert_eq!(flow_screen_image(screen, 6, 1, 2, 2, 0x10_0001), 0);
            assert_eq!(flow_screen_clear(screen, 0, 2, 3, 1, 0x090909), 0);
            assert_eq!(took(flow_screen_show(screen, false)), rust.diff(None, false));
            assert_eq!(took(flow_screen_row(screen, 0)), " hp ·       ");
            // Only what changed after that, and the whole again once forgotten.
            flow_screen_text(screen, 0, 1, c"x".as_ptr(), 0xffffff, -1, false);
            let before = rust.clone();
            rust.put(0, 1, Cell::Text { ch: 'x', fg: [255, 255, 255], bg: None, bold: false });
            assert_eq!(took(flow_screen_show(screen, true)), rust.diff(Some(&before), true));
            assert_eq!(took(flow_screen_show(screen, false)), "\x1b[0m", "nothing changed");
            assert_eq!(flow_screen_forget(screen), 0);
            assert_eq!(took(flow_screen_show(screen, false)), rust.diff(None, false));
            // A ground under what has none, pictures' cells too.
            rust.ground(0, 0, 12, 3, [1, 2, 3]);
            assert_eq!(flow_screen_ground(screen, 0, 0, 12, 3, 0x010203), 0);
            assert_eq!(flow_screen_forget(screen), 0);
            assert_eq!(took(flow_screen_show(screen, false)), rust.diff(None, false));
            assert!(!rust.diff(None, false).contains("\x1b[49m"), "nothing left to the terminal's background");
            flow_screen_free(screen);
        }
    }

    #[test]
    fn what_a_screen_cant_take_is_refused() {
        assert!(flow_screen_new(0, 10).is_null());
        let screen = flow_screen_new(4, 2);
        unsafe {
            assert_eq!(flow_screen_text(screen, 0, 0, std::ptr::null(), 0, -1, false), -1);
            assert_eq!(flow_screen_text(screen, 0, 0, c"\xff".as_ptr(), 0, -1, false), -1, "not UTF-8");
            assert_eq!(flow_screen_text(std::ptr::null_mut(), 0, 0, c"a".as_ptr(), 0, -1, false), -1);
            assert_eq!(flow_screen_text(screen, -3, 7, c"off the screen".as_ptr(), 0, -1, false), 0);
            assert!(flow_screen_row(screen, 2).is_null());
            assert!(flow_screen_show(std::ptr::null_mut(), false).is_null());
            assert_eq!(flow_screen_image(std::ptr::null_mut(), 0, 0, 1, 1, 1), -1);
            assert_eq!(flow_screen_clear(std::ptr::null_mut(), 0, 0, 1, 1, -1), -1);
            assert_eq!(flow_screen_ground(std::ptr::null_mut(), 0, 0, 1, 1, 0), -1);
            assert_eq!(flow_screen_forget(std::ptr::null_mut()), -1);
            flow_screen_free(screen);
            flow_screen_free(std::ptr::null_mut());
        }
    }
}
