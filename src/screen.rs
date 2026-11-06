//! A screen of cells, drawn whole into a buffer each frame and sent to the
//! terminal as its difference from the last one shown: only the cells that
//! changed, the cursor moved only where one doesn't follow on from the one
//! before, and the colors set only where they change. A cell is a letter in
//! its colors or a piece of a picture ([`crate::picture`]'s placeholder
//! cells), so pictures go among the letters and move, scroll and give way
//! to panels as letters do. A letter with no background keeps the
//! terminal's own, which in Flow is the glass and the liquid; a program
//! that wants none left to the terminal, so a light background can't
//! wash it out, lays a ground of its own under the rest
//! ([`Screen::ground`]).
//!
//! Flow Rogue draws its map, sidebar and dialogs this way, its tiles as
//! pictures, and Flow Mines its field, in letters only; [`Spelling`] keeps
//! each sending what it always has.

use std::fmt::Write as _;

use crate::picture::{self, diacritic, Part, Pieces, PLACEHOLDER, PLACEMENT};

/// A color as red, green and blue, 0 to 255.
pub type Rgb = [u8; 3];

/// What one cell of a [`Screen`] shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// A letter in its colors.
    Text {
        /// The letter: a control character shows as a space.
        ch: char,
        /// Its color.
        fg: Rgb,
        /// The color behind it; none leaves the terminal's own background,
        /// which in Flow is the glass and the liquid.
        bg: Option<Rgb>,
        /// Whether it's bold.
        bold: bool,
    },
    /// A cell of a picture, sent as a virtual placement at [`PLACEMENT`]
    /// (`picture::upload(id, image, PLACEMENT, cols, rows)`).
    Image {
        /// The picture.
        id: u32,
        /// The cell's row in it, from 0.
        row: u16,
        /// The cell's column in it, from 0.
        col: u16,
        /// The color behind it, which shows through the picture's clear
        /// pixels where the terminal draws a cell's background under its
        /// picture, as kitty does (Flow draws a picture over its glass
        /// alone, so a program that wants a ground there paints it into
        /// the picture); none leaves the terminal's own.
        bg: Option<Rgb>,
    },
}

impl Cell {
    /// A blank cell on `bg` (none for the terminal's own background).
    pub const fn blank(bg: Option<Rgb>) -> Self {
        Cell::Text { ch: ' ', fg: [200, 200, 200], bg, bold: false }
    }

    /// The color behind the cell, if it has one of its own.
    pub fn bg(&self) -> Option<Rgb> {
        match *self {
            Cell::Text { bg, .. } | Cell::Image { bg, .. } => bg,
        }
    }
}

/// A cell with nothing in it: what a new screen holds, and what a clear
/// leaves.
pub const BLANK: Cell = Cell::blank(None);

/// How [`Screen::diff_as`] writes what changed. The default sends the
/// least: a reset first, so the colors start from the terminal's own, then
/// only the colors and weight that change from cell to cell, and on a
/// screen drawn afresh nothing for blank cells, which the clear blanked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Spelling {
    /// Picture cells go bare, the image's id in its color and the
    /// character alone, as [`crate::detect::Caps::bare`] says they must
    /// (through ConPTY on Windows). Give the screen [`Screen::bare`] first.
    pub bare: bool,
    /// Each change of colors or weight goes as one whole escape from a
    /// reset (`ESC [0;1;38;2;r;g;b;48;2;r;g;b m`), and the colors are reset
    /// after the last cell, rather than a reset first and only what changes
    /// after it. It costs more bytes, and leaves the terminal's colors as
    /// they were. Flow Mines spells its screen so.
    pub whole_styles: bool,
    /// A screen drawn afresh writes its blank cells too, which the clear
    /// before them has already blanked. Flow Mines spells its screen so.
    pub blanks: bool,
}

/// What the game wants on screen: `w` × `h` cells, all [`BLANK`] at first.
/// Writing off its edges does nothing, so a program can draw without
/// clipping first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    /// Its width in cells.
    pub w: u16,
    /// Its height in cells.
    pub h: u16,
    cells: Vec<Cell>,
}

impl Screen {
    /// A blank screen `w` × `h` cells: [`crate::term::size`]'s, say.
    pub fn new(w: u16, h: u16) -> Self {
        Self { w, h, cells: vec![BLANK; w as usize * h as usize] }
    }

    /// The cell at column `x` and row `y`, counting from 0.
    ///
    /// # Panics
    ///
    /// If (`x`, `y`) is off the screen.
    pub fn get(&self, x: u16, y: u16) -> Cell {
        assert!(x < self.w && y < self.h, "cell ({x}, {y}) is off a screen of {} × {}", self.w, self.h);
        self.cells[y as usize * self.w as usize + x as usize]
    }

    /// Sets the cell at (`x`, `y`), if it's on the screen.
    pub fn put(&mut self, x: i32, y: i32, cell: Cell) {
        if x >= 0 && y >= 0 && x < self.w as i32 && y < self.h as i32 {
            self.cells[y as usize * self.w as usize + x as usize] = cell;
        }
    }

    /// Writes `text` from (`x`, `y`) in `fg`, a cell a character, on the
    /// terminal's own background; returns the column after it.
    pub fn text(&mut self, x: i32, y: i32, text: &str, fg: Rgb) -> i32 {
        self.styled(x, y, text, fg, None, false)
    }

    /// [`Screen::text`] in bold.
    pub fn bold(&mut self, x: i32, y: i32, text: &str, fg: Rgb) -> i32 {
        self.styled(x, y, text, fg, None, true)
    }

    /// Writes `text` from (`x`, `y`) in `fg` on `bg` (None for the
    /// terminal's own), bold or not; returns the column after it.
    pub fn styled(&mut self, x: i32, y: i32, text: &str, fg: Rgb, bg: Option<Rgb>, bold: bool) -> i32 {
        let mut x = x;
        for ch in text.chars() {
            self.put(x, y, Cell::Text { ch, fg, bg, bold });
            x += 1;
        }
        x
    }

    /// `text` centered on row `y` between columns `x` and `x + w`, on the
    /// terminal's own background.
    pub fn center(&mut self, x: i32, w: i32, y: i32, text: &str, fg: Rgb, bold: bool) {
        let len = text.chars().count() as i32;
        self.styled(x + (w - len).max(0) / 2, y, text, fg, None, bold);
    }

    /// Picture `id` over `cols` × `rows` cells from (`x`, `y`): send it
    /// first, at [`PLACEMENT`], over as many cells. Each cell keeps the
    /// background of the one it covers, so a picture drawn on a panel is
    /// on the panel's ground.
    pub fn image(&mut self, x: i32, y: i32, cols: u16, rows: u16, id: u32) {
        for row in 0..rows {
            for col in 0..cols {
                let (xx, yy) = (x + col as i32, y + row as i32);
                let bg = self.at(xx, yy).and_then(|cell| cell.bg());
                self.put(xx, yy, Cell::Image { id, row, col, bg });
            }
        }
    }

    /// The cell at (`x`, `y`), if it's on the screen.
    fn at(&self, x: i32, y: i32) -> Option<Cell> {
        (x >= 0 && y >= 0 && x < self.w as i32 && y < self.h as i32).then(|| self.cells[y as usize * self.w as usize + x as usize])
    }

    /// Blanks `w` × `h` cells from (`x`, `y`), on `bg` or (None) the
    /// terminal's own background.
    pub fn clear(&mut self, x: i32, y: i32, w: i32, h: i32, bg: Option<Rgb>) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, Cell::blank(bg));
            }
        }
    }

    /// Lays `bg` under every cell of the `w` × `h` from (`x`, `y`) that has
    /// no background of its own (blank cells, letters drawn on the
    /// terminal's own, pictures' cells), leaving those that have one as
    /// they are. Laid last over the whole screen, it leaves no cell to the
    /// terminal's background, so the screen reads the same over a light
    /// one as a dark; laid first over a part of it (a map, say), it gives
    /// that part a ground of its own.
    pub fn ground(&mut self, x: i32, y: i32, w: i32, h: i32, bg: Rgb) {
        for yy in y.max(0)..(y + h).min(self.h as i32) {
            for xx in x.max(0)..(x + w).min(self.w as i32) {
                let i = yy as usize * self.w as usize + xx as usize;
                match &mut self.cells[i] {
                    Cell::Text { bg: ground @ None, .. } | Cell::Image { bg: ground @ None, .. } => *ground = Some(bg),
                    _ => {}
                }
            }
        }
    }

    /// A frame with rounded corners round `w` × `h` cells from (`x`, `y`),
    /// in `color`, its inside blanked on `bg`.
    pub fn panel(&mut self, x: i32, y: i32, w: i32, h: i32, color: Rgb, bg: Option<Rgb>) {
        self.clear(x, y, w, h, bg);
        let cell = |ch| Cell::Text { ch, fg: color, bg, bold: false };
        for xx in x + 1..x + w - 1 {
            self.put(xx, y, cell('─'));
            self.put(xx, y + h - 1, cell('─'));
        }
        for yy in y + 1..y + h - 1 {
            self.put(x, yy, cell('│'));
            self.put(x + w - 1, yy, cell('│'));
        }
        self.put(x, y, cell('╭'));
        self.put(x + w - 1, y, cell('╮'));
        self.put(x, y + h - 1, cell('╰'));
        self.put(x + w - 1, y + h - 1, cell('╯'));
    }

    /// A bar `w` cells wide from (`x`, `y`), `filled` of it (0 to 1) in
    /// `color`, to an eighth of a cell, on a dark ground.
    pub fn bar(&mut self, x: i32, y: i32, w: i32, filled: f32, color: Rgb) {
        let eighths = (filled.clamp(0.0, 1.0) * w as f32 * 8.0).round() as i32;
        for i in 0..w {
            let n = (eighths - i * 8).clamp(0, 8);
            let ch = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'][n as usize];
            self.put(x + i, y, Cell::Text { ch, fg: color, bg: Some([40, 40, 52]), bold: false });
        }
    }

    /// Row `y`'s letters, a picture's cells as spaces: for tests, and for
    /// reading back what was drawn.
    pub fn row(&self, y: u16) -> String {
        (0..self.w)
            .map(|x| match self.get(x, y) {
                Cell::Text { ch, .. } => ch,
                Cell::Image { .. } => ' ',
            })
            .collect()
    }

    /// Readies the screen's pictures for bare cells
    /// ([`crate::detect::Caps::bare`]), which carry only their image's id
    /// ([`picture::bare_images`], asking `pieces` for the images): each row
    /// of a taller picture shows by its own image, a picture right of its
    /// double by its twin, and a cell that doesn't follow on from the one
    /// to its left in its picture, where something covers part of the
    /// picture, is blanked rather than shown as the picture's first column.
    pub fn bare(&mut self, pieces: &mut impl Pieces) {
        for y in 0..self.h {
            let parts: Vec<Option<Part>> = (0..self.w)
                .map(|x| match self.get(x, y) {
                    Cell::Image { id, row, col, .. } => Some(Part { id, row, col }),
                    Cell::Text { .. } => None,
                })
                .collect();
            for (x, (part, image)) in parts.iter().zip(picture::bare_images(&parts, pieces)).enumerate() {
                // A blanked cell stays on the picture cell's ground.
                let bg = self.get(x as u16, y).bg();
                match (part, image) {
                    (Some(part), Some(image)) => self.put(x as i32, y as i32, Cell::Image { id: image, row: 0, col: part.col, bg }),
                    (Some(_), None) => self.put(x as i32, y as i32, Cell::blank(bg)),
                    (None, _) => {}
                }
            }
        }
    }

    /// The escapes that turn `before` (or, given none or one of another
    /// size, a cleared terminal) into this screen, in [`Spelling`]'s
    /// default, picture cells bare if `bare` ([`crate::detect::Caps::bare`]).
    pub fn diff(&self, before: Option<&Screen>, bare: bool) -> String {
        self.diff_as(before, Spelling { bare, ..Spelling::default() })
    }

    /// [`Screen::diff`], spelled as `spelling` says.
    pub fn diff_as(&self, before: Option<&Screen>, spelling: Spelling) -> String {
        let before = before.filter(|b| (b.w, b.h) == (self.w, self.h));
        let mut out = String::new();
        let mut pen = Pen { bare: spelling.bare, ..Pen::default() };
        if spelling.whole_styles {
            if before.is_none() {
                out.push_str("\x1b[0m\x1b[2J");
            }
        } else {
            out.push_str("\x1b[0m");
            if before.is_none() {
                out.push_str("\x1b[2J");
            }
        }
        let mut at: Option<(u16, u16)> = None;
        for y in 0..self.h {
            for x in 0..self.w {
                let cell = self.get(x, y);
                if before.is_some_and(|b| b.get(x, y) == cell) {
                    continue;
                }
                if before.is_none() && cell == BLANK && !spelling.blanks {
                    continue;
                }
                if at != Some((x, y)) {
                    let _ = write!(out, "\x1b[{};{}H", y + 1, x + 1);
                }
                if spelling.whole_styles {
                    pen.draw_whole(&mut out, cell);
                } else {
                    pen.draw(&mut out, cell);
                }
                at = Some((x + 1, y));
            }
        }
        if spelling.whole_styles && pen.style.is_some() {
            out.push_str("\x1b[0m");
        }
        out
    }
}

/// The colors and weight set on the terminal so far, to send only changes.
#[derive(Default)]
struct Pen {
    fg: Option<Rgb>,
    bg: Option<Rgb>,
    bold: bool,
    underline: Option<Rgb>,
    /// Picture cells carry only their id ([`Spelling::bare`]).
    bare: bool,
    /// The whole style written last, for [`Spelling::whole_styles`]: the
    /// foreground, the background, the weight and the underline's color.
    style: Option<(Rgb, Option<Rgb>, bool, Option<Rgb>)>,
}

/// A number's low 24 bits as a color: a picture's id, or its placement.
fn rgb_of(n: u32) -> Rgb {
    [(n >> 16) as u8, (n >> 8) as u8, n as u8]
}

impl Pen {
    /// Writes `cell`, setting first only what differs from the cell before.
    fn draw(&mut self, out: &mut String, cell: Cell) {
        match cell {
            Cell::Text { ch, fg, bg, bold } => {
                if self.underline.take().is_some() {
                    out.push_str("\x1b[59m");
                }
                if bold != self.bold {
                    out.push_str(if bold { "\x1b[1m" } else { "\x1b[22m" });
                    self.bold = bold;
                }
                self.fg_to(out, fg);
                self.bg_to(out, bg);
                out.push(letter(ch));
            }
            Cell::Image { id, row, col, bg } => {
                if self.bold {
                    out.push_str("\x1b[22m");
                    self.bold = false;
                }
                self.bg_to(out, bg);
                self.fg_to(out, rgb_of(id));
                if self.bare {
                    out.push(PLACEHOLDER);
                    return;
                }
                let placement = rgb_of(PLACEMENT);
                if self.underline != Some(placement) {
                    let [r, g, b] = placement;
                    let _ = write!(out, "\x1b[58;2;{r};{g};{b}m");
                    self.underline = Some(placement);
                }
                out.push(PLACEHOLDER);
                out.extend(diacritic(row));
                out.extend(diacritic(col));
            }
        }
    }

    fn fg_to(&mut self, out: &mut String, fg: Rgb) {
        if self.fg != Some(fg) {
            let [r, g, b] = fg;
            let _ = write!(out, "\x1b[38;2;{r};{g};{b}m");
            self.fg = Some(fg);
        }
    }

    fn bg_to(&mut self, out: &mut String, bg: Option<Rgb>) {
        if bg != self.bg {
            match bg {
                Some([r, g, b]) => {
                    let _ = write!(out, "\x1b[48;2;{r};{g};{b}m");
                }
                None => out.push_str("\x1b[49m"),
            }
            self.bg = bg;
        }
    }

    /// Writes `cell`, its whole style from a reset first if it isn't the
    /// cell before's ([`Spelling::whole_styles`]).
    fn draw_whole(&mut self, out: &mut String, cell: Cell) {
        let style = match cell {
            Cell::Text { fg, bg, bold, .. } => (fg, bg, bold, None),
            Cell::Image { id, bg, .. } => (rgb_of(id), bg, false, (!self.bare).then(|| rgb_of(PLACEMENT))),
        };
        if self.style != Some(style) {
            let (fg, bg, bold, underline) = style;
            out.push_str("\x1b[0");
            if bold {
                out.push_str(";1");
            }
            let _ = write!(out, ";38;2;{};{};{}", fg[0], fg[1], fg[2]);
            if let Some(bg) = bg {
                let _ = write!(out, ";48;2;{};{};{}", bg[0], bg[1], bg[2]);
            }
            if let Some(u) = underline {
                let _ = write!(out, ";58;2;{};{};{}", u[0], u[1], u[2]);
            }
            out.push('m');
            self.style = Some(style);
        }
        match cell {
            Cell::Text { ch, .. } => out.push(letter(ch)),
            Cell::Image { row, col, .. } => {
                out.push(PLACEHOLDER);
                if !self.bare {
                    out.extend(diacritic(row));
                    out.extend(diacritic(col));
                }
            }
        }
    }
}

/// A control character would move the cursor or worse, so it shows as a
/// space.
fn letter(ch: char) -> char {
    if ch.is_control() {
        ' '
    } else {
        ch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_changed_cells_are_sent_and_a_fresh_screen_skips_its_blanks() {
        let mut a = Screen::new(10, 3);
        a.text(0, 0, "hp", [255, 0, 0]);
        assert_eq!(a.diff(None, false), "\x1b[0m\x1b[2J\x1b[1;1H\x1b[38;2;255;0;0mhp");
        let mut b = a.clone();
        b.text(5, 2, "x", [255, 0, 0]);
        assert_eq!(b.diff(Some(&a), false), "\x1b[0m\x1b[3;6H\x1b[38;2;255;0;0mx");
        assert_eq!(b.diff(Some(&b), false), "\x1b[0m", "nothing changed but the reset");
        // Another size is drawn afresh.
        assert!(b.diff(Some(&Screen::new(4, 4)), false).starts_with("\x1b[0m\x1b[2J"));
    }

    #[test]
    fn colors_and_weight_are_set_only_as_they_change() {
        let mut s = Screen::new(6, 1);
        s.styled(0, 0, "ab", [1, 2, 3], Some([4, 5, 6]), true);
        s.text(2, 0, "c", [1, 2, 3]);
        s.put(3, 0, Cell::Text { ch: '\n', fg: [1, 2, 3], bg: None, bold: false });
        assert_eq!(s.diff(Some(&Screen::new(6, 1)), false), "\x1b[0m\x1b[1;1H\x1b[1m\x1b[38;2;1;2;3m\x1b[48;2;4;5;6mab\x1b[22m\x1b[49mc ");
    }

    #[test]
    fn picture_cells_carry_the_id_the_placement_and_their_place_or_go_bare() {
        let mut s = Screen::new(4, 1);
        s.image(0, 0, 2, 1, 0x010203);
        s.bold(2, 0, "!", [9, 9, 9]);
        let d = |n| diacritic(n).unwrap();
        let out = s.diff(Some(&Screen::new(4, 1)), false);
        assert_eq!(out, format!("\x1b[0m\x1b[1;1H\x1b[38;2;1;2;3m\x1b[58;2;0;0;1m{PLACEHOLDER}{}{}{PLACEHOLDER}{}{}\x1b[59m\x1b[1m\x1b[38;2;9;9;9m!", d(0), d(0), d(0), d(1)));
        assert_eq!(s.diff(Some(&Screen::new(4, 1)), true), format!("\x1b[0m\x1b[1;1H\x1b[38;2;1;2;3m{PLACEHOLDER}{PLACEHOLDER}\x1b[1m\x1b[38;2;9;9;9m!"));
        assert_eq!(s.row(0), "  ! ");
    }

    #[test]
    fn whole_styles_reset_each_change_and_leave_the_colors_as_they_were() {
        let whole = Spelling { whole_styles: true, blanks: true, bare: false };
        let mut s = Screen::new(3, 1);
        s.put(0, 0, Cell::Text { ch: 'x', fg: [1, 2, 3], bg: Some([4, 5, 6]), bold: true });
        s.put(1, 0, Cell::Text { ch: 'y', fg: [1, 2, 3], bg: None, bold: false });
        let fresh = s.diff_as(None, whole);
        assert_eq!(fresh, "\x1b[0m\x1b[2J\x1b[1;1H\x1b[0;1;38;2;1;2;3;48;2;4;5;6mx\x1b[0;38;2;1;2;3my\x1b[0;38;2;200;200;200m \x1b[0m", "blanks too");
        assert_eq!(s.diff_as(Some(&s), whole), "", "nothing changed, nothing sent");
        let mut t = s.clone();
        t.image(2, 0, 1, 1, 0x0a0b0c);
        let d = |n| diacritic(n).unwrap();
        assert_eq!(t.diff_as(Some(&s), whole), format!("\x1b[1;3H\x1b[0;38;2;10;11;12;58;2;0;0;1m{PLACEHOLDER}{}{}\x1b[0m", d(0), d(0)));
        assert_eq!(t.diff_as(Some(&s), Spelling { bare: true, ..whole }), format!("\x1b[1;3H\x1b[0;38;2;10;11;12m{PLACEHOLDER}\x1b[0m"));
    }

    #[test]
    fn drawing_off_the_edges_does_nothing() {
        let mut s = Screen::new(5, 3);
        assert_eq!(s.text(3, 1, "abcd", [255; 3]), 7);
        s.text(-2, 0, "xyz", [255; 3]);
        s.text(0, 9, "lost", [255; 3]);
        s.image(4, 2, 3, 3, 7);
        assert_eq!((s.row(0), s.row(1)), ("z    ".to_string(), "   ab".to_string()));
        assert_eq!(s.get(4, 2), Cell::Image { id: 7, row: 0, col: 0, bg: None });
        s.panel(0, 0, 5, 3, [1, 1, 1], None);
        assert_eq!([s.row(0), s.row(1), s.row(2)], ["╭───╮", "│   │", "╰───╯"]);
        s.bar(0, 1, 5, 0.5, [1, 1, 1]);
        assert_eq!(s.row(1), "██▌  ");
        s.center(0, 5, 1, "hi", [1, 1, 1], false);
        assert_eq!(s.row(1), "█hi  ");
    }

    /// Strips and twins as a program might keep them: picture 7 is two
    /// rows high, strips 100 and 101; twins are 1000 on.
    struct Kept;

    impl Pieces for Kept {
        fn strip(&mut self, id: u32, row: u16) -> u32 {
            if id == 7 {
                100 + row as u32
            } else {
                id
            }
        }

        fn twin(&mut self, image: u32) -> u32 {
            1000 + image
        }
    }

    #[test]
    fn bare_pictures_show_by_strips_and_twins_and_a_covered_start_is_blanked() {
        let mut s = Screen::new(8, 2);
        s.image(0, 0, 2, 2, 7);
        s.image(2, 0, 2, 1, 5);
        s.image(4, 0, 2, 1, 5);
        s.image(2, 1, 2, 1, 5);
        s.text(2, 1, "|", [255; 3]);
        s.bare(&mut Kept);
        let at = |x, y| match s.get(x, y) {
            Cell::Image { id, row, col, .. } => Some((id, row, col)),
            Cell::Text { .. } => None,
        };
        assert_eq!([at(0, 0), at(1, 0), at(0, 1), at(1, 1)], [Some((100, 0, 0)), Some((100, 0, 1)), Some((101, 0, 0)), Some((101, 0, 1))]);
        assert_eq!([at(2, 0), at(3, 0), at(4, 0), at(5, 0)], [Some((5, 0, 0)), Some((5, 0, 1)), Some((1005, 0, 0)), Some((1005, 0, 1))]);
        assert_eq!(s.get(3, 1), BLANK, "not its picture's first column");
        // On a ground, a blanked cell keeps it.
        let mut s = Screen::new(4, 1);
        s.image(0, 0, 2, 1, 5);
        s.image(2, 0, 2, 1, 5);
        s.text(2, 0, "|", [255; 3]);
        s.ground(0, 0, 4, 1, [1, 2, 3]);
        s.bare(&mut Kept);
        assert_eq!(s.get(3, 0), Cell::blank(Some([1, 2, 3])));
    }

    #[test]
    fn a_ground_goes_under_every_cell_without_one_and_leaves_the_rest() {
        let mut s = Screen::new(6, 2);
        s.text(0, 0, "ab", [9, 9, 9]);
        s.styled(2, 0, "c", [9, 9, 9], Some([4, 5, 6]), false);
        s.image(3, 0, 2, 1, 7);
        s.ground(0, 0, 6, 1, [1, 2, 3]);
        assert_eq!(s.get(0, 0), Cell::Text { ch: 'a', fg: [9, 9, 9], bg: Some([1, 2, 3]), bold: false });
        assert_eq!(s.get(2, 0).bg(), Some([4, 5, 6]), "its own kept");
        assert_eq!(s.get(3, 0), Cell::Image { id: 7, row: 0, col: 0, bg: Some([1, 2, 3]) });
        assert_eq!((s.get(5, 0), s.get(5, 1)), (Cell::blank(Some([1, 2, 3])), BLANK), "only within its rows");
        // Off the edges it does nothing, and laid again over the whole
        // screen it fills in only what's still left.
        s.ground(-3, 1, 100, 9, [7, 7, 7]);
        s.ground(0, 0, 6, 2, [8, 8, 8]);
        assert!((0..2).all(|y| (0..6).all(|x| s.get(x, y).bg().is_some())));
        assert_eq!((s.get(0, 0).bg(), s.get(0, 1).bg()), (Some([1, 2, 3]), Some([7, 7, 7])));
    }

    #[test]
    fn a_picture_keeps_the_ground_it_covers_and_sends_it_under_its_cells() {
        let mut s = Screen::new(4, 1);
        s.clear(0, 0, 4, 1, Some([4, 5, 6]));
        s.image(1, 0, 2, 1, 0x010203);
        assert_eq!(s.get(1, 0), Cell::Image { id: 0x010203, row: 0, col: 0, bg: Some([4, 5, 6]) });
        let d = |n| diacritic(n).unwrap();
        let out = s.diff(None, false);
        assert_eq!(out, format!("\x1b[0m\x1b[2J\x1b[1;1H\x1b[38;2;200;200;200m\x1b[48;2;4;5;6m \x1b[38;2;1;2;3m\x1b[58;2;0;0;1m{PLACEHOLDER}{}{}{PLACEHOLDER}{}{}\x1b[59m\x1b[38;2;200;200;200m ", d(0), d(0), d(0), d(1)));
        let whole = Spelling { whole_styles: true, blanks: true, bare: true };
        assert_eq!(s.diff_as(Some(&Screen::new(4, 1)), whole), format!("\x1b[1;1H\x1b[0;38;2;200;200;200;48;2;4;5;6m \x1b[0;38;2;1;2;3;48;2;4;5;6m{PLACEHOLDER}{PLACEHOLDER}\x1b[0;38;2;200;200;200;48;2;4;5;6m \x1b[0m"));
    }
}
