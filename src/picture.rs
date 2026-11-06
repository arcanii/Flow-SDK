//! Pictures with the kitty graphics protocol, as Flow shows them. A program
//! sends a picture once, in APC escapes (`ESC _G <keys>;<base64> ESC \`),
//! as a virtual placement (`a=T,U=1`), and from then on shows it with text:
//! Unicode placeholder cells (U+10EEEE) whose foreground color is the
//! image's id and whose underline color is the placement's, with combining
//! marks numbering each cell's row and column in the picture. So a picture
//! scrolls, clears and goes into the scrollback as text does, and showing it
//! somewhere else, or animating it, is only writing cells. Flow keeps every
//! picture this way, the ones it places itself too. The spec:
//! <https://sw.kovidgoyal.net/kitty/graphics-protocol/>.
//!
//! On Windows the commands go down Flow's pipe ([`crate::detect::Terminal::send`])
//! and the cells go bare ([`bare_row`], and [`bare_images`] for pictures
//! among letters), as [`crate::detect::Caps::bare`] says.

use std::fmt::Write as _;

use base64::Engine as _;

/// Kitty's query, which a terminal that shows pictures answers with
/// `ESC _Gi=31;OK ESC \`.
pub const QUERY: &str = "\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\";

/// The character a placeholder cell holds.
pub const PLACEHOLDER: char = '\u{10EEEE}';

/// The placement a picture gets unless the program picks another: cells
/// name it in their underline color, so the terminal finds it at once.
pub const PLACEMENT: u32 = 1;

/// Base64 bytes a command carries at most; a picture larger goes in chunks.
const CHUNK: usize = 4096;

/// Combining marks that number a placeholder's row and column, and carry
/// the top byte of an id past 24 bits: kitty's table
/// (rowcolumn-diacritics.txt), in order.
const DIACRITICS: [u32; 297] = [
    0x0305, 0x030D, 0x030E, 0x0310, 0x0312, 0x033D, 0x033E, 0x033F, 0x0346, 0x034A, 0x034B, 0x034C,
    0x0350, 0x0351, 0x0352, 0x0357, 0x035B, 0x0363, 0x0364, 0x0365, 0x0366, 0x0367, 0x0368, 0x0369,
    0x036A, 0x036B, 0x036C, 0x036D, 0x036E, 0x036F, 0x0483, 0x0484, 0x0485, 0x0486, 0x0487, 0x0592,
    0x0593, 0x0594, 0x0595, 0x0597, 0x0598, 0x0599, 0x059C, 0x059D, 0x059E, 0x059F, 0x05A0, 0x05A1,
    0x05A8, 0x05A9, 0x05AB, 0x05AC, 0x05AF, 0x05C4, 0x0610, 0x0611, 0x0612, 0x0613, 0x0614, 0x0615,
    0x0616, 0x0617, 0x0657, 0x0658, 0x0659, 0x065A, 0x065B, 0x065D, 0x065E, 0x06D6, 0x06D7, 0x06D8,
    0x06D9, 0x06DA, 0x06DB, 0x06DC, 0x06DF, 0x06E0, 0x06E1, 0x06E2, 0x06E4, 0x06E7, 0x06E8, 0x06EB,
    0x06EC, 0x0730, 0x0732, 0x0733, 0x0735, 0x0736, 0x073A, 0x073D, 0x073F, 0x0740, 0x0741, 0x0743,
    0x0745, 0x0747, 0x0749, 0x074A, 0x07EB, 0x07EC, 0x07ED, 0x07EE, 0x07EF, 0x07F0, 0x07F1, 0x07F3,
    0x0816, 0x0817, 0x0818, 0x0819, 0x081B, 0x081C, 0x081D, 0x081E, 0x081F, 0x0820, 0x0821, 0x0822,
    0x0823, 0x0825, 0x0826, 0x0827, 0x0829, 0x082A, 0x082B, 0x082C, 0x082D, 0x0951, 0x0953, 0x0954,
    0x0F82, 0x0F83, 0x0F86, 0x0F87, 0x135D, 0x135E, 0x135F, 0x17DD, 0x193A, 0x1A17, 0x1A75, 0x1A76,
    0x1A77, 0x1A78, 0x1A79, 0x1A7A, 0x1A7B, 0x1A7C, 0x1B6B, 0x1B6D, 0x1B6E, 0x1B6F, 0x1B70, 0x1B71,
    0x1B72, 0x1B73, 0x1CD0, 0x1CD1, 0x1CD2, 0x1CDA, 0x1CDB, 0x1CE0, 0x1DC0, 0x1DC1, 0x1DC3, 0x1DC4,
    0x1DC5, 0x1DC6, 0x1DC7, 0x1DC8, 0x1DC9, 0x1DCB, 0x1DCC, 0x1DD1, 0x1DD2, 0x1DD3, 0x1DD4, 0x1DD5,
    0x1DD6, 0x1DD7, 0x1DD8, 0x1DD9, 0x1DDA, 0x1DDB, 0x1DDC, 0x1DDD, 0x1DDE, 0x1DDF, 0x1DE0, 0x1DE1,
    0x1DE2, 0x1DE3, 0x1DE4, 0x1DE5, 0x1DE6, 0x1DFE, 0x20D0, 0x20D1, 0x20D4, 0x20D5, 0x20D6, 0x20D7,
    0x20DB, 0x20DC, 0x20E1, 0x20E7, 0x20E9, 0x20F0, 0x2CEF, 0x2CF0, 0x2CF1, 0x2DE0, 0x2DE1, 0x2DE2,
    0x2DE3, 0x2DE4, 0x2DE5, 0x2DE6, 0x2DE7, 0x2DE8, 0x2DE9, 0x2DEA, 0x2DEB, 0x2DEC, 0x2DED, 0x2DEE,
    0x2DEF, 0x2DF0, 0x2DF1, 0x2DF2, 0x2DF3, 0x2DF4, 0x2DF5, 0x2DF6, 0x2DF7, 0x2DF8, 0x2DF9, 0x2DFA,
    0x2DFB, 0x2DFC, 0x2DFD, 0x2DFE, 0x2DFF, 0xA66F, 0xA67C, 0xA67D, 0xA6F0, 0xA6F1, 0xA8E0, 0xA8E1,
    0xA8E2, 0xA8E3, 0xA8E4, 0xA8E5, 0xA8E6, 0xA8E7, 0xA8E8, 0xA8E9, 0xA8EA, 0xA8EB, 0xA8EC, 0xA8ED,
    0xA8EE, 0xA8EF, 0xA8F0, 0xA8F1, 0xAAB0, 0xAAB2, 0xAAB3, 0xAAB7, 0xAAB8, 0xAABE, 0xAABF, 0xAAC1,
    0xFE20, 0xFE21, 0xFE22, 0xFE23, 0xFE24, 0xFE25, 0xFE26, 0x10A0F, 0x10A38, 0x1D185, 0x1D186, 0x1D187,
    0x1D188, 0x1D189, 0x1D1AA, 0x1D1AB, 0x1D1AC, 0x1D1AD, 0x1D242, 0x1D243, 0x1D244,
];

/// The most rows or columns a picture's placeholder cells can number.
pub const MAX_CELLS: u16 = DIACRITICS.len() as u16;

/// The mark that numbers row or column `n`, if there's one.
pub fn diacritic(n: u16) -> Option<char> {
    DIACRITICS.get(n as usize).and_then(|&c| char::from_u32(c))
}

/// How a picture's pixels are given: `f=24`, `f=32` or `f=100`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Three bytes a pixel.
    Rgb,
    /// Four bytes a pixel, straight alpha.
    Rgba,
    /// A PNG file's bytes.
    Png,
}

impl Format {
    /// Its number in the command's `f`.
    pub fn code(self) -> u32 {
        match self {
            Format::Rgb => 24,
            Format::Rgba => 32,
            Format::Png => 100,
        }
    }
}

/// A picture to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// How its pixels are given.
    pub format: Format,
    /// Its width in pixels.
    pub width: u32,
    /// Its height in pixels.
    pub height: u32,
    /// Rows of pixels top first, 3 or 4 bytes each with straight alpha; or
    /// a PNG file's bytes.
    pub data: Vec<u8>,
}

impl Image {
    /// `width` × `height` pixels, four bytes each with straight alpha,
    /// rows top first.
    pub fn rgba(width: u32, height: u32, data: Vec<u8>) -> Self {
        debug_assert_eq!(data.len(), width as usize * height as usize * 4);
        Self { format: Format::Rgba, width, height, data }
    }

    /// `width` × `height` pixels, three bytes each, rows top first.
    pub fn rgb(width: u32, height: u32, data: Vec<u8>) -> Self {
        debug_assert_eq!(data.len(), width as usize * height as usize * 3);
        Self { format: Format::Rgb, width, height, data }
    }

    /// A PNG file's bytes, its size read from its header (0 × 0 if it
    /// isn't one; the terminal reads the size itself either way).
    pub fn png(data: Vec<u8>) -> Self {
        let be = |i: usize| data.get(i..i + 4).map_or(0, |b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
        let (width, height) = if data.starts_with(b"\x89PNG\r\n\x1a\n") { (be(16), be(20)) } else { (0, 0) };
        Self { format: Format::Png, width, height, data }
    }

    /// RGBA that's opaque throughout as RGB: a quarter fewer bytes to send.
    pub fn packed(self) -> Self {
        let pixels = self.data.as_chunks::<4>().0;
        if self.format != Format::Rgba || !pixels.iter().all(|p| p[3] == 255) {
            return self;
        }
        let data = pixels.iter().flat_map(|p| [p[0], p[1], p[2]]).collect();
        Self { format: Format::Rgb, data, ..self }
    }

    /// Row `row` of `rows` the picture is shown over: the pixels between its
    /// edges, rounded; none for a PNG. Where cells go bare
    /// ([`crate::detect::Caps::bare`]) each row of a taller picture has to
    /// be an image of its own.
    pub fn strip(&self, row: u16, rows: u16) -> Option<Self> {
        let px = match self.format {
            Format::Rgb => 3,
            Format::Rgba => 4,
            Format::Png => return None,
        };
        let h = self.height as usize;
        let edge = |r: u16| ((r as f32 * h as f32 / rows.max(1) as f32).round() as usize).min(h);
        let top = edge(row).min(h.saturating_sub(1));
        let bottom = edge(row + 1).max(top + 1).min(h);
        let line = self.width as usize * px;
        let data = self.data.get(top * line..bottom * line)?.to_vec();
        Some(Self { format: self.format, width: self.width, height: (bottom - top) as u32, data })
    }
}

/// The commands that send `image` as image `id`, shown by placeholder cells
/// over `cols` × `rows` cells as placement `placement` (a virtual placement,
/// `a=T,U=1`): raw pixels zlib-compressed (`o=z`), in chunks of 4096 base64
/// bytes (`m=1` while more follow). `q=2` keeps the terminal from
/// answering. Sending an id again replaces its picture, and cells that show
/// it show the new one.
pub fn upload(id: u32, image: &Image, placement: u32, cols: u16, rows: u16) -> String {
    upload_as(id, image, placement, cols, rows, Chunks::Quiet)
}

/// What a picture's chunks after the first carry besides `m`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Chunks {
    /// `q=2` too, so no chunk is answered: as [`upload`] writes them.
    #[default]
    Quiet,
    /// `m` alone, the least kitty's spec asks of them: Flow goes by the
    /// first chunk's `q` for the whole picture, though other terminals may
    /// not.
    Short,
}

/// [`upload`], its chunks after the first written as `chunks` says.
pub fn upload_as(id: u32, image: &Image, placement: u32, cols: u16, rows: u16, chunks: Chunks) -> String {
    upload_packed(id, &image.pack(), placement, cols, rows, chunks)
}

/// A picture made ready to send: its pixels compressed (raw ones) and
/// base64-encoded, the slow part of sending it. [`upload_packed`] writes
/// its commands once its id is known, so a program making many pictures at
/// once can pack them on threads of its own first ([`Image::pack`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packed {
    /// How its pixels were given.
    pub format: Format,
    /// Its width in pixels.
    pub width: u32,
    /// Its height in pixels.
    pub height: u32,
    /// The payload, base64.
    pub data: String,
}

impl Image {
    /// Its pixels made ready to send.
    pub fn pack(&self) -> Packed {
        Packed { format: self.format, width: self.width, height: self.height, data: payload(self) }
    }
}

/// [`upload_as`] for a picture already packed.
pub fn upload_packed(id: u32, packed: &Packed, placement: u32, cols: u16, rows: u16, chunks: Chunks) -> String {
    let keys = format!("a=T,U=1,{},i={id},p={placement},c={cols},r={rows},q=2", pixel_keys(packed.format, packed.width, packed.height));
    commands(&keys, &packed.data, chunks)
}

/// The commands that send `image` as image `id` and only keep it (`a=t`),
/// for a program that places it later with its own commands.
pub fn transmit(id: u32, image: &Image) -> String {
    commands(&format!("a=t,{},i={id},q=2", pixel_keys(image.format, image.width, image.height)), &payload(image), Chunks::Quiet)
}

/// Forgets image `id` and its pixels (`d=I`).
pub fn delete(id: u32) -> String {
    format!("\x1b_Ga=d,d=I,i={id},q=2\x1b\\")
}

/// Takes away every picture on the screen in use, keeping their pixels
/// (`d=a`); Flow keeps those in the scrollback and on the other screen.
pub const DELETE_SHOWN: &str = "\x1b_Ga=d,d=a,q=2\x1b\\";

fn pixel_keys(format: Format, width: u32, height: u32) -> String {
    match format {
        Format::Png => "f=100".to_string(),
        f => format!("f={},o=z,s={width},v={height}", f.code()),
    }
}

fn payload(image: &Image) -> String {
    let bytes = match image.format {
        Format::Png => image.data.clone(),
        _ => miniz_oxide::deflate::compress_to_vec_zlib(&image.data, 6),
    };
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// `keys` and `data` as one command, or as chunks: the first with the keys,
/// the rest with only `m`, and `q` unless they're `Short`.
fn commands(keys: &str, data: &str, rest: Chunks) -> String {
    let chunks: Vec<&[u8]> = data.as_bytes().chunks(CHUNK).collect();
    let mut out = String::with_capacity(data.len() + 128);
    for (i, chunk) in chunks.iter().enumerate() {
        let more = u8::from(i + 1 < chunks.len());
        if i == 0 {
            let _ = write!(out, "\x1b_G{keys},m={more};");
        } else if rest == Chunks::Short {
            let _ = write!(out, "\x1b_Gm={more};");
        } else {
            let _ = write!(out, "\x1b_Gm={more},q=2;");
        }
        out.push_str(std::str::from_utf8(chunk).unwrap_or_default());
        out.push_str("\x1b\\");
    }
    if chunks.is_empty() {
        let _ = write!(out, "\x1b_G{keys},m=0;\x1b\\");
    }
    out
}

/// The color escape that names `n` (24 bits of it) with `sgr`: 38 the
/// foreground, 58 the underline.
fn color(out: &mut String, sgr: u8, n: u32) {
    let _ = write!(out, "\x1b[{sgr};2;{};{};{}m", (n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff);
}

/// Row `row` of a picture's cells, `cols` wide from column `from`: each a
/// placeholder with its row's and column's marks (and the id's top byte as
/// a third, past 24 bits), in the id's color and underlined in the
/// placement's, then the colors back to the terminal's own. Move the
/// cursor to where the row goes first.
pub fn placeholder_row(id: u32, placement: u32, row: u16, from: u16, cols: u16) -> String {
    let mut out = String::new();
    color(&mut out, 38, id);
    if placement != 0 {
        color(&mut out, 58, placement);
    }
    let high = (id >> 24 != 0).then(|| diacritic((id >> 24) as u16)).flatten();
    for col in from..from.saturating_add(cols) {
        out.push(PLACEHOLDER);
        out.extend(diacritic(row));
        out.extend(diacritic(col));
        out.extend(high);
    }
    out.push_str(if placement != 0 { "\x1b[39;59m" } else { "\x1b[39m" });
    out
}

/// A whole picture's cells, `cols` × `rows` with its top-left at (`col`,
/// `row`) on the screen (counting from 0), each row placed by the cursor.
pub fn placeholders(id: u32, placement: u32, (col, row): (u16, u16), cols: u16, rows: u16) -> String {
    (0..rows).map(|r| format!("\x1b[{};{}H{}", row + r + 1, col + 1, placeholder_row(id, placement, r, 0, cols))).collect()
}

/// A row of bare placeholder cells, `cols` wide: the id's color (24 bits)
/// and the character alone, as ConPTY passes them ([`crate::detect::Caps::bare`]).
/// Flow takes a bare cell's column from the cell to its left, one more when
/// that shows the same image, and its row as the first, so each row of a
/// taller picture is an image of its own ([`Image::strip`]), and a picture
/// right of its double needs an id of its own.
pub fn bare_row(id: u32, cols: u16) -> String {
    let mut out = String::new();
    color(&mut out, 38, id);
    out.extend(std::iter::repeat_n(PLACEHOLDER, cols as usize));
    out.push_str("\x1b[39m");
    out
}

/// A cell showing part of a picture: which picture, and the cell's row and
/// column in it, counting from 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Part {
    /// The picture.
    pub id: u32,
    /// The cell's row in it.
    pub row: u16,
    /// The cell's column in it.
    pub col: u16,
}

/// The images a program sent for its pictures, as [`bare_images`] asks for
/// them where cells go bare.
pub trait Pieces {
    /// The image that shows row `row` of picture `id` alone: one of its
    /// strips ([`Image::strip`]), or the picture itself when it's a row high.
    fn strip(&mut self, id: u32, row: u16) -> u32;
    /// Another image with `image`'s pixels, for a picture whose first
    /// column comes right after its double: sent the first time it's asked
    /// for, and the same one after that.
    fn twin(&mut self, image: u32) -> u32;
}

/// For a row of the screen where cells go bare ([`crate::detect::Caps::bare`]):
/// the image each cell has to name for Flow to show `parts` (a picture's
/// part, or none where the row holds something else), or none where no
/// image can, and the cell is best left blank. Flow takes a bare cell's
/// column as one more than the cell to its left's when that names the same
/// image, else as the first, and its row as the first. So a picture's first
/// column names the strip of its row, the cells after it follow on, a
/// picture right after its double names a twin, and a cell that doesn't
/// follow on from the one to its left in its picture (where something
/// covers part of the picture) can't be shown.
pub fn bare_images(parts: &[Option<Part>], pieces: &mut impl Pieces) -> Vec<Option<u32>> {
    // The cell to the left: the part it shows, and the image it names.
    let mut left: Option<(Part, u32)> = None;
    parts
        .iter()
        .map(|part| {
            let Some(part) = *part else {
                left = None;
                return None;
            };
            let image = match left {
                Some((l, image)) if (l.id, l.row, l.col + 1) == (part.id, part.row, part.col) => Some(image),
                _ if part.col == 0 => {
                    let strip = pieces.strip(part.id, part.row);
                    Some(if left.is_some_and(|l| l.1 == strip) { pieces.twin(strip) } else { strip })
                }
                _ => None,
            };
            left = image.map(|image| (part, image));
            image
        })
        .collect()
}

/// Ids for a program's pictures, from a random place in the 24 bits a
/// placeholder's color holds, so they're unlikely to be ones another
/// program in the same terminal chose: Flow keeps a tab's pictures in one
/// store.
#[derive(Debug, Clone)]
pub struct Ids {
    next: u32,
}

impl Ids {
    /// The first id given, leaving the low numbers, which programs that
    /// choose their own ids tend to pick, alone.
    pub const FIRST: u32 = 0x10_0000;
    /// The last id given, the most a color holds.
    pub const LAST: u32 = 0xFF_FFFF;

    /// Starting at a place `seed` picks: the time, say, or the process's id.
    pub fn new(seed: u32) -> Self {
        Self { next: Self::FIRST + seed % (Self::LAST - Self::FIRST) }
    }

    /// Going on after `last`, for a program that picks where its ids start
    /// itself: the first is the one after it, and they stay in the same
    /// range as [`Ids::new`]'s.
    pub fn after(last: u32) -> Self {
        Self { next: last.clamp(Self::FIRST, Self::LAST) }
    }

    /// The next id, round to [`Ids::FIRST`] again after [`Ids::LAST`].
    pub fn next_id(&mut self) -> u32 {
        self.next = if self.next >= Self::LAST { Self::FIRST } else { self.next + 1 };
        self.next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_sent_once_as_a_virtual_placement() {
        let image = Image::rgba(20, 20, vec![255; 20 * 20 * 4]).packed();
        assert_eq!(image.format, Format::Rgb, "opaque throughout, it goes as RGB");
        let sent = upload(0x123456, &image, PLACEMENT, 2, 1);
        assert!(sent.starts_with("\x1b_Ga=T,U=1,f=24,o=z,s=20,v=20,i=1193046,p=1,c=2,r=1,q=2,m=0;"), "{sent:.80}");
        assert!(sent.ends_with("\x1b\\") && sent.matches("\x1b_G").count() == 1);
        let png = Image::png(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\x01\0\0\0\0\x80".to_vec());
        assert_eq!((png.width, png.height), (256, 128));
        assert!(transmit(7, &png).starts_with("\x1b_Ga=t,f=100,i=7,q=2,m=0;"));
    }

    #[test]
    fn large_pictures_go_in_chunks_that_say_more_follows() {
        // Noise doesn't compress, so it takes several chunks.
        let mut x = 1u32;
        let px: Vec<u8> = (0..64 * 64 * 4)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect();
        let sent = upload(9, &Image::rgba(64, 64, px), 1, 8, 4);
        let parts: Vec<&str> = sent.split("\x1b\\").filter(|p| !p.is_empty()).collect();
        assert!(parts.len() > 2);
        assert!(parts[0].contains(",m=1;"));
        assert!(parts[1].starts_with("\x1b_Gm=1,q=2;"));
        assert!(parts.last().unwrap().starts_with("\x1b_Gm=0,q=2;"));
        assert!(parts.iter().all(|p| p.len() <= CHUNK + 80));
    }

    #[test]
    fn cells_carry_the_id_the_placement_and_their_place() {
        let row = placeholder_row(0x010203, 1, 2, 0, 2);
        let d = |n| diacritic(n).unwrap();
        assert_eq!(row, format!("\x1b[38;2;1;2;3m\x1b[58;2;0;0;1m{PLACEHOLDER}{}{}{PLACEHOLDER}{}{}\x1b[39;59m", d(2), d(0), d(2), d(1)));
        // Past 24 bits, the id's top byte rides as a third mark.
        assert!(placeholder_row(0x0500_0001, 1, 0, 0, 1).contains(&format!("{PLACEHOLDER}{}{}{}", d(0), d(0), d(5))));
        // Bare, a cell is the id's color and the character alone.
        assert_eq!(bare_row(0x010203, 2), format!("\x1b[38;2;1;2;3m{PLACEHOLDER}{PLACEHOLDER}\x1b[39m"));
        assert!(placeholders(5, 1, (3, 4), 1, 2).starts_with("\x1b[5;4H"));
        assert_eq!(diacritic(0), Some('\u{0305}'));
        assert_eq!(diacritic(MAX_CELLS), None);
    }

    #[test]
    fn a_strip_is_one_rows_pixels() {
        let image = Image::rgb(2, 10, (0..60).collect());
        let strips: Vec<Image> = (0..3).map(|r| image.strip(r, 3).unwrap()).collect();
        assert_eq!(strips.iter().map(|s| s.height).sum::<u32>(), 10);
        assert_eq!(strips[0].data[..6], [0, 1, 2, 3, 4, 5]);
        assert_eq!(strips[2].data.last(), Some(&59));
    }

    #[test]
    fn ids_stay_in_the_24_bits_a_color_holds() {
        let mut ids = Ids::new(u32::MAX);
        for _ in 0..10 {
            let id = ids.next_id();
            assert!((Ids::FIRST..=Ids::LAST).contains(&id));
        }
        // Going on after one the program picked, and round again at the top.
        assert_eq!(Ids::after(0x20_0000).next_id(), 0x20_0001);
        let mut top = Ids::after(Ids::LAST);
        assert_eq!((top.next_id(), top.next_id()), (Ids::FIRST, Ids::FIRST + 1));
        assert_eq!(Ids::after(5).next_id(), Ids::FIRST + 1, "kept in range");
    }

    #[test]
    fn later_chunks_can_carry_m_alone() {
        let mut x = 7u32;
        let px: Vec<u8> = (0..64 * 64 * 4)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect();
        let image = Image::rgba(64, 64, px);
        let quiet = upload(9, &image, 1, 8, 4);
        assert_eq!(upload_as(9, &image, 1, 8, 4, Chunks::Quiet), quiet);
        let short = upload_as(9, &image, 1, 8, 4, Chunks::Short);
        let parts: Vec<&str> = short.split("\x1b\\").filter(|p| !p.is_empty()).collect();
        assert!(parts.len() > 2);
        assert_eq!(parts[0], quiet.split("\x1b\\").next().unwrap(), "the first has every key");
        assert!(parts[1].starts_with("\x1b_Gm=1;") && parts.last().unwrap().starts_with("\x1b_Gm=0;"));
        assert_eq!(short.replace("\x1b_Gm=1;", "\x1b_Gm=1,q=2;").replace("\x1b_Gm=0;", "\x1b_Gm=0,q=2;"), quiet);
        // Packed first, on another thread say, it's the same.
        let packed = image.pack();
        assert_eq!((packed.format, packed.width, packed.height), (Format::Rgba, 64, 64));
        assert_eq!(upload_packed(9, &packed, 1, 8, 4, Chunks::Short), short);
    }

    /// Strips and twins as a program might keep them: a picture two rows
    /// high is strips 100 and 101, twins are 1000 on.
    #[derive(Default)]
    struct Kept {
        twins: Vec<u32>,
    }

    impl Pieces for Kept {
        fn strip(&mut self, id: u32, row: u16) -> u32 {
            if id == 7 {
                100 + row as u32
            } else {
                id
            }
        }

        fn twin(&mut self, image: u32) -> u32 {
            if !self.twins.contains(&image) {
                self.twins.push(image);
            }
            1000 + image
        }
    }

    #[test]
    fn bare_cells_name_strips_twins_and_blanks() {
        let part = |id, row, col| Some(Part { id, row, col });
        let mut kept = Kept::default();
        // A tall picture's second row, then a tile twice over, then the tile
        // with its first column covered by a letter.
        let row = [part(7, 1, 0), part(7, 1, 1), part(5, 0, 0), part(5, 0, 1), part(5, 0, 0), part(5, 0, 1), None, part(5, 0, 1), part(5, 0, 0)];
        assert_eq!(bare_images(&row, &mut kept), [Some(101), Some(101), Some(5), Some(5), Some(1005), Some(1005), None, None, Some(5)]);
        assert_eq!(kept.twins, [5]);
        // Its double after a twin is the first again.
        let row = [part(5, 0, 0), part(5, 0, 0), part(5, 0, 0)];
        assert_eq!(bare_images(&row, &mut kept), [Some(5), Some(1005), Some(5)]);
    }
}
