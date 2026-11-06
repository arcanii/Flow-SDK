//! 3D models with the Ratty graphics protocol, as Flow speaks it (and Ratty,
//! which began it, so a program written for one works in both):
//! `ESC _ ratty;g;<verb>;<key=value>… ESC \`. `s` asks what's supported,
//! `r` registers an object by an id (Wavefront `.obj` or binary glTF
//! `.glb`, sent in base64, in chunks or whole, or named by a path), `p`
//! places it centered on a cell over `w` × `h` cells, `u` changes how a
//! placed object looks, and `d` deletes one or all. Placing it again moves
//! it, and registering it again under a placed id draws the new model in
//! the old one's place: a program moves a whole scene that way each frame
//! ([`crate::stage`]). The spec:
//! <https://docs.rs/crate/ratty/0.4.1/source/protocols/graphics.md>.
//!
//! Flow fits a model to its cells by its box (centered, the longest side
//! fitted, with a tenth's margin), lights it from above left, and keeps it
//! in the terminal as a picture shown by placeholder cells, so it scrolls
//! and clears as text does.

use std::fmt::Write as _;

use base64::Engine as _;

/// Ratty's `s`, which a terminal that shows models answers with
/// `ESC _ratty;g;s;v=1;fmt=obj|glb;… ESC \`.
pub const QUERY: &str = "\x1b_ratty;g;s\x1b\\";

/// The most bytes a model may be in Flow.
pub const MAX_BYTES: usize = 64 << 20;
/// The most cells a placement may cover a side in Flow.
pub const MAX_SPAN: u16 = 200;
/// The most pixels a placement's picture may be a side in Flow.
pub const MAX_SIDE: u32 = 4096;
/// The most pixels a tab's placements may cover all told in Flow.
pub const MAX_PIXELS: u64 = 8 << 20;
/// The most placements Flow shows at once in a tab.
pub const MAX_SHOWN: usize = 64;
/// The most objects Flow keeps for a tab.
pub const MAX_OBJECTS: usize = 256;

/// The scale that undoes the tenth's margin a model is fitted with, so its
/// box is its cells: `1 / 0.9`.
pub const FILL: f32 = 1.0 / 0.9;

/// A model's file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Wavefront OBJ, as text.
    Obj,
    /// Binary glTF.
    Glb,
}

impl Format {
    /// Its name in the escape's `fmt`: `obj` or `glb`.
    pub fn name(self) -> &'static str {
        match self {
            Format::Obj => "obj",
            Format::Glb => "glb",
        }
    }
}

/// Registers `bytes` as object `id` in one command. `name` is the file's
/// name, for the terminal's messages.
pub fn register(id: u32, format: Format, bytes: &[u8], name: Option<&str>) -> String {
    register_chunks(id, format, bytes, name, usize::MAX)
}

/// Registers `bytes` as object `id` in commands of `chunk` bytes at most
/// (3072 is 4 KB of base64), each saying whether more follow; the last
/// loads the model.
pub fn register_chunks(id: u32, format: Format, bytes: &[u8], name: Option<&str>, chunk: usize) -> String {
    let fmt = format.name();
    let name = name.map(|n| format!("name={n};")).unwrap_or_default();
    let parts: Vec<&[u8]> = if bytes.is_empty() { vec![bytes] } else { bytes.chunks(chunk.max(1)).collect() };
    let mut out = String::new();
    for (i, part) in parts.iter().enumerate() {
        let more = u8::from(i + 1 < parts.len());
        let data = base64::engine::general_purpose::STANDARD.encode(part);
        let _ = write!(out, "\x1b_ratty;g;r;id={id};fmt={fmt};source=payload;more={more};{name}{data}\x1b\\");
    }
    out
}

/// Registers the file at `path` as object `id`. Flow asks the user first,
/// naming the tab and the file, and reads it only once they allow it; a
/// relative path is from the shell's folder.
pub fn register_path(id: u32, format: Format, path: &str) -> String {
    format!("\x1b_ratty;g;r;id={id};fmt={};path={path}\x1b\\", format.name())
}

/// How a placed object looks, as `p` sets it and `u` changes it; what's
/// left as None the terminal keeps (or takes as its default).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Look {
    /// Spins, tumbles and bobs.
    pub animate: Option<bool>,
    /// Larger or smaller than its cells, 1 fitting them with a tenth's
    /// margin ([`FILL`] fills them).
    pub scale: Option<f32>,
    /// Thickness for flat artwork, which also turns to an oblique view.
    pub depth: Option<f32>,
    /// Tints an OBJ, as 0xRRGGBB (a glTF has its own colors).
    pub color: Option<u32>,
    /// Lighter above 1, darker below.
    pub brightness: Option<f32>,
    /// In pixels, y up; z only moves it nearer or further.
    pub offset: [Option<f32>; 3],
    /// Degrees about x, y and z, turned in that order.
    pub rotation: [Option<f32>; 3],
    /// Stretched along x, y and z.
    pub scale3: [Option<f32>; 3],
}

impl Look {
    fn write(&self, out: &mut String) {
        if let Some(a) = self.animate {
            let _ = write!(out, ";animate={}", u8::from(a));
        }
        if let Some(v) = self.scale {
            let _ = write!(out, ";scale={v}");
        }
        if let Some(v) = self.depth {
            let _ = write!(out, ";depth={v}");
        }
        if let Some(c) = self.color {
            let _ = write!(out, ";color={:06x}", c & 0xff_ffff);
        }
        if let Some(v) = self.brightness {
            let _ = write!(out, ";brightness={v}");
        }
        for (keys, values) in [(["px", "py", "pz"], self.offset), (["rx", "ry", "rz"], self.rotation), (["sx", "sy", "sz"], self.scale3)] {
            for (key, value) in keys.iter().zip(values) {
                if let Some(v) = value {
                    let _ = write!(out, ";{key}={v}");
                }
            }
        }
    }
}

/// Where object `id` goes: centered on screen cell (`row`, `col`), counting
/// from 0, over `cols` × `rows` cells (each at most [`MAX_SPAN`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    /// The object placed.
    pub id: u32,
    /// The row of the cell it's centered on.
    pub row: u16,
    /// The column of the cell it's centered on.
    pub col: u16,
    /// How many cells wide it's fitted to.
    pub cols: u16,
    /// How many cells high it's fitted to.
    pub rows: u16,
    /// How it looks.
    pub look: Look,
}

impl Place {
    /// Object `id` centered on cell (`row`, `col`) over `cols` × `rows`
    /// cells, looking as it was.
    pub fn new(id: u32, (row, col): (u16, u16), (cols, rows): (u16, u16)) -> Self {
        Self { id, row, col, cols, rows, look: Look::default() }
    }

    /// Over the `cols` × `rows` cells whose top-left is (`top`, `left`):
    /// centered on the middle cell, or the one past the middle.
    pub fn over(id: u32, (top, left): (u16, u16), (cols, rows): (u16, u16)) -> Self {
        Self::new(id, (top + rows.saturating_sub(1).div_ceil(2), left + cols.saturating_sub(1).div_ceil(2)), (cols, rows))
    }

    /// Looking as `look` says.
    pub fn look(self, look: Look) -> Self {
        Self { look, ..self }
    }

    /// The escape that places it.
    pub fn escape(&self) -> String {
        let Self { id, row, col, cols, rows, .. } = self;
        let mut out = format!("\x1b_ratty;g;p;id={id};row={row};col={col};w={cols};h={rows}");
        self.look.write(&mut out);
        out.push_str("\x1b\\");
        out
    }
}

/// Changes how placed object `id` looks, without placing it again.
pub fn update(id: u32, look: &Look) -> String {
    let mut out = format!("\x1b_ratty;g;u;id={id}");
    look.write(&mut out);
    out.push_str("\x1b\\");
    out
}

/// Deletes object `id`, and takes its placement away.
pub fn delete(id: u32) -> String {
    format!("\x1b_ratty;g;d;id={id}\x1b\\")
}

/// Deletes every object the program's tab holds.
pub const DELETE_ALL: &str = "\x1b_ratty;g;d\x1b\\";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_is_registered_whole_or_in_chunks() {
        assert_eq!(register(1, Format::Glb, b"abc", Some("scene.glb")), "\x1b_ratty;g;r;id=1;fmt=glb;source=payload;more=0;name=scene.glb;YWJj\x1b\\");
        let chunks = register_chunks(3, Format::Obj, b"abcdefg", None, 3);
        assert_eq!(chunks.matches("more=1").count(), 2);
        assert!(chunks.ends_with("more=0;Zw==\x1b\\"), "{chunks}");
        assert_eq!(register_path(2, Format::Obj, "rat.obj"), "\x1b_ratty;g;r;id=2;fmt=obj;path=rat.obj\x1b\\");
    }

    #[test]
    fn a_placement_writes_only_what_its_look_sets() {
        let place = Place::over(1, (2, 4), (80, 23)).look(Look { scale: Some(FILL), ..Look::default() });
        assert_eq!(place.escape(), "\x1b_ratty;g;p;id=1;row=13;col=44;w=80;h=23;scale=1.1111112\x1b\\");
        let look = Look { animate: Some(true), color: Some(0x7ce7ff), rotation: [None, Some(30.0), None], ..Look::default() };
        assert_eq!(update(4, &look), "\x1b_ratty;g;u;id=4;animate=1;color=7ce7ff;ry=30\x1b\\");
        assert_eq!(delete(4), "\x1b_ratty;g;d;id=4\x1b\\");
    }
}
