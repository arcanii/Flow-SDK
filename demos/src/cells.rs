//! A screen of cells drawn in a program's own memory and written to the
//! terminal as escapes, a row at a time and only the rows that changed
//! since the last frame, each color written only where it changes. A cell
//! with no background shows Flow's glass through it.

/// A character in a color (0xRRGGBB), over a background or over nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: u32,
    pub bg: Option<u32>,
    pub bold: bool,
}

impl Cell {
    pub const BLANK: Cell = Cell { ch: ' ', fg: 0xffffff, bg: None, bold: false };

    pub fn new(ch: char, fg: u32) -> Self {
        Self { ch, fg, ..Self::BLANK }
    }
}

/// What a frame shows, `cols` × `rows` cells.
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    pub cols: u16,
    pub rows: u16,
    cells: Vec<Cell>,
}

impl Grid {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self { cols, rows, cells: vec![Cell::BLANK; usize::from(cols) * usize::from(rows)] }
    }

    pub fn get(&self, col: u16, row: u16) -> Option<Cell> {
        (col < self.cols && row < self.rows).then(|| self.cells[usize::from(row) * usize::from(self.cols) + usize::from(col)])
    }

    /// Puts a cell; one off the screen is let go.
    pub fn put(&mut self, col: u16, row: u16, cell: Cell) {
        if col < self.cols && row < self.rows {
            self.cells[usize::from(row) * usize::from(self.cols) + usize::from(col)] = cell;
        }
    }

    /// `text` from `col` in `fg`, keeping the cells' backgrounds, cut at
    /// the edge; returns the column after it.
    pub fn text(&mut self, col: u16, row: u16, text: &str, fg: u32, bold: bool) -> u16 {
        let mut at = col;
        for ch in text.chars() {
            if let Some(cell) = self.get(at, row) {
                self.put(at, row, Cell { ch, fg, bold, ..cell });
            }
            at = at.saturating_add(1);
        }
        at
    }

    /// `text` in the middle of `row`; if it's wider than the screen, its
    /// start shows.
    pub fn centered(&mut self, row: u16, text: &str, fg: u32, bold: bool) {
        let width = text.chars().count();
        let col = usize::from(self.cols).saturating_sub(width) / 2;
        self.text(col as u16, row, text, fg, bold);
    }

    /// Gives `w` × `h` cells from (`col`, `row`) the background `bg`,
    /// keeping what's written on them.
    pub fn fill(&mut self, col: u16, row: u16, w: u16, h: u16, bg: Option<u32>) {
        for r in row..row.saturating_add(h) {
            for c in col..col.saturating_add(w) {
                if let Some(cell) = self.get(c, r) {
                    self.put(c, r, Cell { bg, ..cell });
                }
            }
        }
    }

    /// A row's text, for tests and for finding things on it.
    pub fn line(&self, row: u16) -> String {
        (0..self.cols).filter_map(|c| self.get(c, row)).map(|c| c.ch).collect()
    }

    /// The row as escapes: to its start, then its cells.
    fn row_escape(&self, row: u16) -> String {
        let mut out = format!("\x1b[{};1H\x1b[0m", row + 1);
        let (mut fg, mut bg, mut bold): (Option<u32>, Option<u32>, bool) = (None, None, false);
        for col in 0..self.cols {
            let Some(cell) = self.get(col, row) else { break };
            if cell.ch != ' ' && fg != Some(cell.fg) {
                let c = cell.fg;
                out += &format!("\x1b[38;2;{};{};{}m", (c >> 16) & 0xff, (c >> 8) & 0xff, c & 0xff);
                fg = Some(cell.fg);
            }
            if cell.ch != ' ' && cell.bold != bold {
                out += if cell.bold { "\x1b[1m" } else { "\x1b[22m" };
                bold = cell.bold;
            }
            if cell.bg != bg {
                match cell.bg {
                    Some(c) => out += &format!("\x1b[48;2;{};{};{}m", (c >> 16) & 0xff, (c >> 8) & 0xff, c & 0xff),
                    None => out += "\x1b[49m",
                }
                bg = cell.bg;
            }
            out.push(cell.ch);
        }
        out + "\x1b[0m"
    }
}

/// What was last written, to write only what changed.
#[derive(Debug, Default)]
pub struct Screen {
    size: Option<(u16, u16)>,
    rows: Vec<String>,
}

impl Screen {
    /// What brings the terminal from what it showed to `grid`: all of it,
    /// over a cleared screen, the first time and when the size changes.
    pub fn write(&mut self, grid: &Grid) -> String {
        let mut out = String::new();
        if self.size != Some((grid.cols, grid.rows)) {
            self.size = Some((grid.cols, grid.rows));
            self.rows = vec![String::new(); usize::from(grid.rows)];
            out += "\x1b[0m\x1b[2J";
        }
        for row in 0..grid.rows {
            let line = grid.row_escape(row);
            if self.rows[usize::from(row)] != line {
                out += &line;
                self.rows[usize::from(row)] = line;
            }
        }
        out
    }
}

/// A color from hue (0 to 1 round the wheel), saturation and value.
pub fn hsv(h: f32, s: f32, v: f32) -> u32 {
    let h = h.rem_euclid(1.0) * 6.0;
    let (i, f) = (h.floor() as u32 % 6, h.fract());
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    let byte = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u32;
    byte(r) << 16 | byte(g) << 8 | byte(b)
}

/// `c` with each channel times `k`.
pub fn scale(c: u32, k: f32) -> u32 {
    let ch = |s: u32| ((((c >> s) & 0xff) as f32 * k).round().clamp(0.0, 255.0) as u32) << s;
    ch(16) | ch(8) | ch(0)
}

/// From `a` toward `b`, `t` of the way (0xRRGGBB both).
pub fn blend(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |s: u32| {
        let (x, y) = (((a >> s) & 0xff) as f32, ((b >> s) & 0xff) as f32);
        ((x + (y - x) * t).round() as u32) << s
    };
    ch(16) | ch(8) | ch(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_rows_that_changed_are_written_again() {
        let mut grid = Grid::new(20, 3);
        grid.text(2, 1, "hello", 0xff0000, false);
        grid.fill(0, 2, 4, 1, Some(0x102030));
        let mut screen = Screen::default();
        let first = screen.write(&grid);
        assert!(first.starts_with("\x1b[0m\x1b[2J"));
        assert!(first.contains("\x1b[2;1H\x1b[0m  \x1b[38;2;255;0;0mhello"), "{first:?}");
        assert!(first.contains("\x1b[48;2;16;32;48m    \x1b[49m"), "a background only where it's asked for");
        assert_eq!(screen.write(&grid), "", "nothing changed");
        grid.text(2, 1, "j", 0xff0000, true);
        let again = screen.write(&grid);
        assert!(again.starts_with("\x1b[2;1H") && !again.contains("\x1b[1;1H") && again.contains("\x1b[1mj\x1b[22mello"), "{again:?}");
        assert!(screen.write(&Grid::new(21, 3)).starts_with("\x1b[0m\x1b[2J"), "all of it after a resize");
    }

    #[test]
    fn text_is_cut_at_the_edge_and_centered_where_it_fits() {
        let mut grid = Grid::new(10, 1);
        assert_eq!(grid.text(7, 0, "abcdef", 0xffffff, false), 13);
        assert_eq!(grid.line(0), "       abc");
        grid.centered(0, "xy", 0xffffff, false);
        assert_eq!(grid.line(0), "    xy abc");
    }

    #[test]
    fn colors_go_round_the_wheel_and_blend() {
        assert_eq!((hsv(0.0, 1.0, 1.0), hsv(1.0 / 3.0, 1.0, 1.0), hsv(2.0 / 3.0, 1.0, 1.0)), (0xff0000, 0x00ff00, 0x0000ff));
        assert_eq!(hsv(0.5, 0.0, 0.5), 0x808080);
        assert_eq!(scale(0x804020, 0.5), 0x402010);
        assert_eq!(blend(0x000000, 0xff8040, 0.5), 0x804020);
        assert_eq!(blend(0x123456, 0xffffff, 0.0), 0x123456);
    }
}
