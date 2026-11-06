//! Teletext's mosaics as Unicode has them, which Flow draws itself (rather
//! than taking them from the font) so that cells side by side join: a cell
//! parted into 2 × 3 pixels, the sextants (U+1FB00 on, with the halves and
//! the full block among the block elements) and the separated ones, each
//! pixel with a gap left of and below it (U+1CE51 on). A picture is drawn
//! in teletext's eight colors on [`Pixels`] and turned into cells as
//! teletext would show it: in each cell one color over another, or over
//! nothing, where Flow's glass shows through.

/// Teletext's eight colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

impl Color {
    pub const ALL: [Color; 8] = [Color::Black, Color::Red, Color::Green, Color::Yellow, Color::Blue, Color::Magenta, Color::Cyan, Color::White];

    pub fn rgb(self) -> u32 {
        match self {
            Color::Black => 0x000000,
            Color::Red => 0xff0000,
            Color::Green => 0x00ff00,
            Color::Yellow => 0xffff00,
            Color::Blue => 0x0000ff,
            Color::Magenta => 0xff00ff,
            Color::Cyan => 0x00ffff,
            Color::White => 0xffffff,
        }
    }
}

/// The sextant whose pixels `mask` names: bit 0 the top left, then across
/// and down to bit 5 the bottom right. Unicode leaves out the empty cell,
/// the left and right halves and the full block, which are a space and
/// three block elements.
pub fn sextant(mask: u8) -> char {
    match mask & 63 {
        0 => ' ',
        21 => '\u{258C}',
        42 => '\u{2590}',
        63 => '\u{2588}',
        m => char::from_u32(0x1FB00 + u32::from(m) - 1 - u32::from(m > 21) - u32::from(m > 42)).unwrap_or(' '),
    }
}

/// The separated mosaic whose pixels `mask` names, as [`sextant`] counts
/// them: teletext's separated graphics, each pixel apart from the next.
pub fn separated(mask: u8) -> char {
    match mask & 63 {
        0 => ' ',
        m => char::from_u32(0x1CE50 + u32::from(m)).unwrap_or(' '),
    }
}

/// A cell of a teletext page: a character in a color, over a background,
/// or over nothing, where the glass shows through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Option<Color>,
}

impl Cell {
    pub const BLANK: Cell = Cell { ch: ' ', fg: Color::White, bg: None };

    /// The same pixels in separated graphics, if it's a sextant.
    pub fn separated(self) -> Self {
        match (0..64u8).find(|&m| sextant(m) == self.ch) {
            Some(mask) => Cell { ch: separated(mask), ..self },
            None => self,
        }
    }
}

/// A picture in mosaic pixels, two across and three down a cell, each a
/// color or none.
#[derive(Debug, Clone, PartialEq)]
pub struct Pixels {
    pub width: usize,
    pub height: usize,
    px: Vec<Option<Color>>,
}

impl Pixels {
    /// A clear picture over `cols` × `rows` cells.
    pub fn new(cols: usize, rows: usize) -> Self {
        Self { width: cols * 2, height: rows * 3, px: vec![None; cols * 2 * rows * 3] }
    }

    pub fn get(&self, x: i32, y: i32) -> Option<Color> {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return None;
        }
        self.px[y as usize * self.width + x as usize]
    }

    /// Sets a pixel; one off the picture is let go.
    pub fn set(&mut self, x: i32, y: i32, c: Option<Color>) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.px[y as usize * self.width + x as usize] = c;
        }
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Option<Color>) {
        for y in y..y + h {
            for x in x..x + w {
                self.set(x, y, c);
            }
        }
    }

    /// A round blot of radius `r` about (x, y), squashed `squash` times
    /// down, since a mosaic pixel is taller than it's wide.
    pub fn disc(&mut self, x: f32, y: f32, r: f32, squash: f32, c: Option<Color>) {
        let (x0, x1) = ((x - r).floor() as i32, (x + r).ceil() as i32);
        let (y0, y1) = ((y - r * squash).floor() as i32, (y + r * squash).ceil() as i32);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let (dx, dy) = (px as f32 + 0.5 - x, (py as f32 + 0.5 - y) / squash);
                if dx * dx + dy * dy <= r * r {
                    self.set(px, py, c);
                }
            }
        }
    }

    /// `text` in the big letters ([`glyph`]), `scale` pixels to a dot, its
    /// top left at (x, y), a dot's width between letters; returns the x
    /// after it.
    pub fn letters(&mut self, x: i32, y: i32, text: &str, scale: i32, c: Color) -> i32 {
        let mut at = x;
        for ch in text.chars() {
            let Some(rows) = glyph(ch) else { continue };
            for (dy, line) in rows.iter().enumerate() {
                for (dx, dot) in line.chars().enumerate() {
                    if dot == '#' {
                        self.rect(at + dx as i32 * scale, y + dy as i32 * scale, scale, scale, Some(c));
                    }
                }
            }
            at += (rows[0].len() as i32 + 1) * scale;
        }
        at
    }

    /// The cells that show it, row by row, as teletext would: one color
    /// over another in each. Where a pixel is clear the cell is the
    /// commonest color over nothing, where the glass shows through; else
    /// over the first of `grounds` (the colors things stand on) in it, or
    /// over its second commonest color. A third color in a cell takes the
    /// first's, as teletext had to, so that shapes keep their pixels, if
    /// not their colors. A cell all of one color is a full block rather
    /// than a background, which Flow draws a little see-through.
    pub fn cells(&self, grounds: &[Color]) -> Vec<Vec<Cell>> {
        let (cols, rows) = (self.width / 2, self.height / 3);
        (0..rows)
            .map(|row| {
                (0..cols)
                    .map(|col| {
                        let px: Vec<Option<Color>> = (0..6).map(|i| self.get(col as i32 * 2 + i % 2, row as i32 * 3 + i / 2)).collect();
                        let mut counts: Vec<(Color, usize)> = Vec::new();
                        for c in px.iter().flatten() {
                            match counts.iter_mut().find(|(k, _)| k == c) {
                                Some((_, n)) => *n += 1,
                                None => counts.push((*c, 1)),
                            }
                        }
                        // The commonest first, ties to the first seen.
                        counts.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
                        let ground = grounds.iter().copied().find(|g| counts.iter().any(|(c, _)| c == g));
                        let (fg, bg) = match ground {
                            _ if px.contains(&None) => (counts.first().map(|&(c, _)| c), None),
                            Some(g) => (counts.iter().map(|&(c, _)| c).find(|&c| c != g), Some(g)),
                            None => (counts.first().map(|&(c, _)| c), counts.get(1).map(|&(c, _)| c)),
                        };
                        let Some(fg) = fg else { return Cell { ch: ' ', fg: Color::White, bg } };
                        let mask = px.iter().enumerate().filter(|(_, &p)| p.is_some() && p != bg).fold(0u8, |m, (i, _)| m | 1 << i);
                        Cell { ch: sextant(mask), fg, bg }
                    })
                    .collect()
            })
            .collect()
    }
}

/// The big letters' dots, five across and seven down (a colon one across):
/// the clock's digits and the logo's letters.
pub fn glyph(c: char) -> Option<[&'static str; 7]> {
    Some(match c {
        '0' => [".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###."],
        '1' => ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."],
        '2' => [".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"],
        '3' => ["#####", "...#.", "..#..", "...#.", "....#", "#...#", ".###."],
        '4' => ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."],
        '5' => ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."],
        '6' => ["..##.", ".#...", "#....", "####.", "#...#", "#...#", ".###."],
        '7' => ["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."],
        '8' => [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."],
        '9' => [".###.", "#...#", "#...#", ".####", "....#", "...#.", ".##.."],
        ':' => [".", ".", "#", ".", "#", ".", "."],
        ' ' => [".", ".", ".", ".", ".", ".", "."],
        'F' => ["#####", "#....", "#....", "####.", "#....", "#....", "#...."],
        'L' => ["#....", "#....", "#....", "#....", "#....", "#....", "#####"],
        'O' => [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
        'W' => ["#...#", "#...#", "#...#", "#.#.#", "#.#.#", "##.##", "#...#"],
        _ => return None,
    })
}

/// How many pixels across `text` takes in the big letters at `scale`, the
/// gap after the last left out.
pub fn letters_width(text: &str, scale: i32) -> i32 {
    let dots: i32 = text.chars().filter_map(glyph).map(|g| g[0].len() as i32 + 1).sum();
    (dots - 1).max(0) * scale
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pixels a block character fills, as Flow's own `mosaic.rs`
    /// reads them.
    fn mask_of(c: char) -> Option<u8> {
        let u = c as u32;
        match u {
            0x20 => Some(0),
            0x258C => Some(21),
            0x2590 => Some(42),
            0x2588 => Some(63),
            0x1FB00..=0x1FB3B => {
                let mut m = (u - 0x1FB00 + 1) as u8;
                if m >= 21 {
                    m += 1;
                }
                if m >= 42 {
                    m += 1;
                }
                Some(m)
            }
            0x1CE51..=0x1CE8F => Some((u - 0x1CE50) as u8),
            _ => None,
        }
    }

    #[test]
    fn every_mask_is_the_sextant_flow_reads_back() {
        for mask in 0..64u8 {
            assert_eq!(mask_of(sextant(mask)), Some(mask), "sextant {mask}");
            assert_eq!(mask_of(separated(mask)), Some(mask), "separated {mask}");
        }
        assert_eq!((sextant(1), sextant(62)), ('\u{1FB00}', '\u{1FB3B}'));
        assert_eq!(separated(63), '\u{1CE8F}');
    }

    #[test]
    fn a_cell_shows_one_color_over_another_or_over_the_glass() {
        let mut p = Pixels::new(5, 1);
        // Red over nothing in the top row of the first cell.
        p.rect(0, 0, 2, 1, Some(Color::Red));
        // Blue throughout the second, a yellow pixel on it.
        p.rect(2, 0, 2, 3, Some(Color::Blue));
        p.set(3, 2, Some(Color::Yellow));
        // Mostly yellow on blue, which is what things stand on.
        p.rect(4, 0, 2, 3, Some(Color::Yellow));
        p.set(4, 0, Some(Color::Blue));
        // All yellow.
        p.rect(6, 0, 2, 3, Some(Color::Yellow));
        // Over nothing, a third color takes the commonest's, keeping its shape.
        let mut q = Pixels::new(1, 1);
        q.rect(0, 1, 2, 1, Some(Color::Cyan));
        q.rect(0, 2, 1, 1, Some(Color::Blue));
        assert_eq!(q.cells(&[Color::Blue])[0][0], Cell { ch: sextant(0b011100), fg: Color::Cyan, bg: None });
        let cells = &p.cells(&[Color::Blue])[0];
        assert_eq!(cells[0], Cell { ch: sextant(0b11), fg: Color::Red, bg: None });
        assert_eq!(cells[1], Cell { ch: sextant(0b100000), fg: Color::Yellow, bg: Some(Color::Blue) });
        assert_eq!(cells[2], Cell { ch: sextant(0b111110), fg: Color::Yellow, bg: Some(Color::Blue) });
        assert_eq!(cells[3], Cell { ch: '\u{2588}', fg: Color::Yellow, bg: None }, "a full block, not a background");
        assert_eq!(cells[4], Cell::BLANK);
        assert_eq!(p.cells(&[])[0][1], Cell { ch: sextant(0b011111), fg: Color::Blue, bg: Some(Color::Yellow) }, "with no ground, the commonest over the next");
        assert_eq!(cells[0].separated().ch, separated(0b11));
    }

    #[test]
    fn big_letters_are_five_dots_wide_and_their_width_is_counted() {
        for c in "0123456789FLOW".chars() {
            let g = glyph(c).expect("drawn");
            assert!(g.iter().all(|line| line.len() == 5), "{c}");
        }
        assert_eq!(letters_width("12:30", 2), (6 + 6 + 2 + 6 + 6 - 1) * 2);
        let mut p = Pixels::new(20, 4);
        let end = p.letters(0, 0, "FLOW", 1, Color::White);
        assert_eq!(end, letters_width("FLOW", 1) + 1);
        assert_eq!(p.get(0, 6), Some(Color::White), "F's stem runs down");
        assert_eq!(p.get(6, 6), Some(Color::White), "L's foot");
    }
}
