//! Pictures from C, with the kitty graphics protocol as Flow shows them
//! ([`flow_sdk::picture`]): a picture sent once as a virtual placement and
//! shown by placeholder cells, which scroll and clear as text does. The
//! escapes come back as strings; [`flow_picture_show`] does the whole of
//! it, sending the picture where it goes and writing its cells, bare ones
//! on Windows too.

use std::ffi::{c_char, c_int};
use std::io::{self, Write};

use flow_sdk::picture::{self, Ids, Image, PLACEMENT};

use crate::term::FlowTerminal;
use crate::{guard, owned_or_null, slice};

/// `FLOW_PIXELS_*`: how a picture's bytes are given, as kitty's `f` key.
pub const PIXELS_RGB: i32 = 24;
pub const PIXELS_RGBA: i32 = 32;
pub const PIXELS_PNG: i32 = 100;

/// A picture to send. Raw pixels are rows top first, 3 or 4 bytes each
/// (RGBA with straight alpha), `len` exactly `width × height` times that; a
/// PNG is its file's bytes, its size read from its header.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FlowImage {
    /// `FLOW_PIXELS_*`.
    pub format: i32,
    pub width: u32,
    pub height: u32,
    pub data: *const u8,
    pub len: usize,
}

/// The SDK's picture for `image`, or None if its bytes don't fit what it
/// says. RGBA that's opaque throughout goes as RGB, a quarter fewer bytes.
pub(crate) unsafe fn image(image: *const FlowImage) -> Option<Image> {
    let image = image.as_ref()?;
    let data = slice(image.data, image.len)?;
    let pixels = (image.width as usize).checked_mul(image.height as usize);
    let fits = |bytes: usize| image.width > 0 && image.height > 0 && pixels.and_then(|p| p.checked_mul(bytes)) == Some(data.len());
    match image.format {
        PIXELS_RGB if fits(3) => Some(Image::rgb(image.width, image.height, data.to_vec())),
        PIXELS_RGBA if fits(4) => Some(Image::rgba(image.width, image.height, data.to_vec()).packed()),
        PIXELS_PNG if !data.is_empty() => Some(Image::png(data.to_vec())),
        _ => None,
    }
}

/// An id to start a program's pictures from, picked by `seed` (the
/// process's id, say) in the 24 bits a cell's color holds; count up from it
/// for more, staying at or under 0xFFFFFF.
#[no_mangle]
pub extern "C" fn flow_picture_id(seed: u32) -> u32 {
    guard(0x10_0000, || Ids::new(seed).next_id())
}

#[no_mangle]
pub unsafe extern "C" fn flow_picture_upload(id: u32, img: *const FlowImage, placement: u32, cols: u16, rows: u16) -> *mut c_char {
    owned_or_null(|| Some(picture::upload(id, &image(img)?, placement, cols, rows)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_picture_transmit(id: u32, img: *const FlowImage) -> *mut c_char {
    owned_or_null(|| Some(picture::transmit(id, &image(img)?)))
}

#[no_mangle]
pub extern "C" fn flow_picture_delete(id: u32) -> *mut c_char {
    owned_or_null(|| Some(picture::delete(id)))
}

#[no_mangle]
pub extern "C" fn flow_picture_delete_shown() -> *mut c_char {
    owned_or_null(|| Some(picture::DELETE_SHOWN.to_string()))
}

#[no_mangle]
pub extern "C" fn flow_picture_placeholder_row(id: u32, placement: u32, row: u16, from: u16, cols: u16) -> *mut c_char {
    owned_or_null(|| Some(picture::placeholder_row(id, placement, row, from, cols)))
}

#[no_mangle]
pub extern "C" fn flow_picture_placeholders(id: u32, placement: u32, col: u16, row: u16, cols: u16, rows: u16) -> *mut c_char {
    // The cursor's rows and columns count from 1, which the last has to
    // leave room for.
    let fits = u32::from(row) + u32::from(rows) <= u32::from(u16::MAX) && col < u16::MAX;
    owned_or_null(|| fits.then(|| picture::placeholders(id, placement, (col, row), cols, rows)))
}

#[no_mangle]
pub extern "C" fn flow_picture_bare_row(id: u32, cols: u16) -> *mut c_char {
    owned_or_null(|| Some(picture::bare_row(id, cols)))
}

/// What showing `image` as `id` over `cols` × `rows` cells sends and
/// writes: the commands (one picture, or on bare cells one a row, ids `id`
/// up) and the cells, at (`col`, `row`) or, with none, at the cursor a line
/// each. None for a PNG on bare cells, which can't be cut into rows, or ids
/// past the 24 bits a color holds.
pub(crate) fn show(bare: bool, id: u32, image: &Image, cols: u16, rows: u16, at: Option<(u16, u16)>) -> Option<(Vec<String>, String, u32)> {
    let mut cells = String::new();
    let mut line = |r: u16, text: String| match at {
        Some((col, row)) => cells.push_str(&format!("\x1b[{};{}H{text}", u32::from(row) + u32::from(r) + 1, u32::from(col) + 1)),
        None => cells.push_str(&format!("{text}\r\n")),
    };
    if bare {
        if id.checked_add(u32::from(rows))? > 0xFF_FFFF + 1 {
            return None;
        }
        let mut commands = Vec::new();
        for r in 0..rows {
            let strip = image.strip(r, rows)?;
            commands.push(picture::upload(id + u32::from(r), &strip, PLACEMENT, cols, 1));
            line(r, picture::bare_row(id + u32::from(r), cols));
        }
        Some((commands, cells, u32::from(rows)))
    } else {
        for r in 0..rows {
            line(r, picture::placeholder_row(id, PLACEMENT, r, 0, cols));
        }
        Some((vec![picture::upload(id, image, PLACEMENT, cols, rows)], cells, 1))
    }
}

/// Sends `image` as picture `id` where the terminal takes pictures and
/// writes the cells that show it over `cols` × `rows` cells: with its
/// top-left at (`col`, `row`), or where the cursor is, a line each, if
/// either is negative. The number of ids it took (1, or `rows` on bare
/// cells), or -1.
#[no_mangle]
pub unsafe extern "C" fn flow_picture_show(terminal: *mut FlowTerminal, id: u32, img: *const FlowImage, cols: u16, rows: u16, col: i32, row: i32) -> c_int {
    guard(-1, || {
        let (Some(terminal), Some(image)) = (terminal.as_mut(), image(img)) else { return -1 };
        let at = match (u16::try_from(col), u16::try_from(row)) {
            (Ok(col), Ok(row)) => Some((col, row)),
            _ if col < 0 || row < 0 => None,
            _ => return -1,
        };
        let Some((commands, cells, ids)) = show(terminal.0.caps.bare, id, &image, cols, rows, at) else { return -1 };
        for command in &commands {
            if terminal.0.send(command.as_bytes()).is_err() {
                return -1;
            }
        }
        let mut out = io::stdout().lock();
        if out.write_all(cells.as_bytes()).and_then(|_| out.flush()).is_err() {
            return -1;
        }
        ids as c_int
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    fn taken(s: *mut c_char) -> String {
        assert!(!s.is_null());
        // SAFETY: handed out by this crate, freed once.
        unsafe {
            let text = CStr::from_ptr(s).to_str().unwrap().to_string();
            crate::flow_free(s.cast());
            text
        }
    }

    #[test]
    fn a_picture_goes_as_the_sdk_sends_it() {
        let pixels = vec![255u8; 4 * 2 * 4];
        let img = FlowImage { format: PIXELS_RGBA, width: 4, height: 2, data: pixels.as_ptr(), len: pixels.len() };
        let sent = taken(unsafe { flow_picture_upload(0x123456, &img, 1, 2, 1) });
        assert_eq!(sent, picture::upload(0x123456, &Image::rgba(4, 2, pixels.clone()).packed(), 1, 2, 1));
        assert!(sent.contains("f=24"), "opaque throughout, it goes as RGB");
        assert_eq!(taken(flow_picture_delete(7)), picture::delete(7));
        assert_eq!(taken(flow_picture_delete_shown()), picture::DELETE_SHOWN);
        assert_eq!(taken(flow_picture_placeholder_row(5, 1, 2, 0, 3)), picture::placeholder_row(5, 1, 2, 0, 3));
        assert_eq!(taken(flow_picture_placeholders(5, 1, 3, 4, 2, 2)), picture::placeholders(5, 1, (3, 4), 2, 2));
        assert_eq!(taken(flow_picture_bare_row(5, 3)), picture::bare_row(5, 3));
        assert!((0x10_0000..=0xFF_FFFF).contains(&flow_picture_id(42)));
    }

    #[test]
    fn pixels_that_dont_fit_their_size_are_refused() {
        let pixels = [0u8; 10];
        let short = FlowImage { format: PIXELS_RGB, width: 2, height: 2, data: pixels.as_ptr(), len: pixels.len() };
        let unknown = FlowImage { format: 7, width: 1, height: 1, data: pixels.as_ptr(), len: 1 };
        let empty = FlowImage { format: PIXELS_PNG, width: 0, height: 0, data: std::ptr::null(), len: 0 };
        let null = FlowImage { format: PIXELS_RGB, width: 1, height: 1, data: std::ptr::null(), len: 3 };
        unsafe {
            for img in [&short, &unknown, &empty, &null] {
                assert!(flow_picture_upload(1, img, 1, 1, 1).is_null());
            }
            assert!(flow_picture_transmit(1, std::ptr::null()).is_null());
            assert_eq!(flow_picture_show(std::ptr::null_mut(), 1, &short, 1, 1, 0, 0), -1);
        }
        assert!(flow_picture_placeholders(1, 1, 0, u16::MAX - 1, 1, 2).is_null(), "rows past the last");
    }

    #[test]
    fn bare_cells_take_a_picture_a_row_and_the_rest_one() {
        let image = Image::rgb(2, 4, (0..24).collect());
        let (commands, cells, ids) = show(false, 9, &image, 2, 2, Some((3, 1))).unwrap();
        assert_eq!((commands.len(), ids), (1, 1));
        assert!(cells.starts_with("\x1b[2;4H") && cells.contains("\x1b[3;4H"));
        let (commands, cells, ids) = show(true, 9, &image, 2, 2, None).unwrap();
        assert_eq!((commands.len(), ids), (2, 2));
        assert_eq!(commands[1], picture::upload(10, &image.strip(1, 2).unwrap(), PLACEMENT, 2, 1));
        assert_eq!(cells, format!("{}\r\n{}\r\n", picture::bare_row(9, 2), picture::bare_row(10, 2)));
        assert!(show(true, 9, &Image::png(vec![1, 2, 3]), 2, 2, None).is_none(), "a PNG can't be cut into rows");
        assert!(show(true, 0xFF_FFFF, &image, 2, 2, None).is_none(), "ids past 24 bits");
    }
}
