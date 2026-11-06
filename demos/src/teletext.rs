//! Teletext: pages of it in the screen's own cells, as a television's
//! decoder showed Ceefax and Oracle, forty columns across. Every line is
//! one of DEC's double lines, so forty columns fill an 80-column window:
//! twice as wide (`ESC # 6`), or the halves of a line twice as tall too
//! (`ESC # 3` over `ESC # 4`, each with the same text), which Flow draws
//! from the font at twice its size. Pictures are teletext's mosaics
//! ([`crate::mosaic`]) in its eight colors, over nothing where the liquid
//! shows through.
//!
//! The header counts pages as they come round while one is found, and
//! shows the time; digits pick a page, ← and → step, the Fastext row's
//! four colors (r, g, y, c, or a click) go to theirs, h holds a page's
//! turn through its subpages and ? reveals what's concealed. Live: the
//! index's wave and its flashing NEW, the news turning over, a clock, and
//! a boat at sea. A page arriving splashes the liquid in its band's color,
//! the clock ripples it each minute, and the boat's bow splashes it.

use std::time::Duration;

use flow_sdk::fx::{Effect, Sound};
use flow_sdk::input::KeyCode;
use flow_sdk::term;

use crate::clock::{self, Time};
use crate::mosaic::{self, Cell, Color, Pixels};
use crate::{TextDemo, Written};

/// A page's columns and rows.
pub const COLS: usize = 40;
pub const ROWS: usize = 25;
/// The row teletext keeps blank over the Fastext links, left out on a
/// screen only 24 rows high; and the links' row.
const SPARE: usize = 23;
const FASTEXT: usize = 24;

/// Seconds a page takes to come round once asked for, and when stepped to.
const SEARCH: f32 = 0.9;
const STEP: f32 = 0.35;
/// Seconds each subpage shows before the next.
const SUBPAGE: f32 = 10.0;
/// Flashing text shows for this much of each second.
const FLASH_ON: f32 = 0.66;
/// Teletext was still: ten frames a second are plenty for its waves.
const INTERVAL: Duration = Duration::from_millis(100);
/// How much taller than wide a mosaic pixel is on a double-width line, or
/// rather how much wider: a cell is about twice as tall as wide, doubled
/// it's square, and parted 2 × 3 its pixels are half again as wide.
const SQUASH: f32 = 1.5;

/// How a line is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    /// Twice as wide (`ESC # 6`).
    Wide,
    /// The top half of a line twice as wide and twice as tall (`ESC # 3`).
    Top,
    /// Its bottom half (`ESC # 4`), with the same text.
    Bottom,
}

impl Size {
    fn escape(self) -> &'static str {
        match self {
            Size::Wide => "\x1b#6",
            Size::Top => "\x1b#3",
            Size::Bottom => "\x1b#4",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub size: Size,
    pub cells: [Cell; COLS],
}

/// A page of 25 rows of 40 cells.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub rows: Vec<Row>,
}

impl Page {
    pub fn blank() -> Self {
        Self { rows: vec![Row { size: Size::Wide, cells: [Cell::BLANK; COLS] }; ROWS] }
    }

    /// Makes `row` and the next the halves of a line twice as tall: what's
    /// put on the first goes on both.
    pub fn tall(&mut self, row: usize) {
        if row + 1 < ROWS {
            self.rows[row].size = Size::Top;
            self.rows[row + 1].size = Size::Bottom;
        }
    }

    /// Puts a cell, on both halves of a tall line.
    pub fn put(&mut self, row: usize, col: usize, cell: Cell) {
        if row >= ROWS || col >= COLS {
            return;
        }
        self.rows[row].cells[col] = cell;
        if self.rows[row].size == Size::Top && row + 1 < ROWS {
            self.rows[row + 1].cells[col] = cell;
        }
    }

    /// `text` from `col` in `fg` over `bg`, cut at the page's edge.
    pub fn text(&mut self, row: usize, col: usize, text: &str, fg: Color, bg: Option<Color>) {
        for (i, ch) in text.chars().enumerate() {
            self.put(row, col + i, Cell { ch, fg, bg });
        }
    }

    /// The row's background across the page.
    pub fn band(&mut self, row: usize, bg: Color) {
        for col in 0..COLS {
            let cell = self.rows[row].cells[col];
            self.put(row, col, Cell { bg: Some(bg), ..cell });
        }
    }

    /// A picture's cells with its top left at (`col`, `row`).
    pub fn picture(&mut self, row: usize, col: usize, cells: &[Vec<Cell>]) {
        for (r, line) in cells.iter().enumerate() {
            for (c, &cell) in line.iter().enumerate() {
                self.put(row + r, col + c, cell);
            }
        }
    }

    /// The text of a row, for tests and for finding things on it.
    pub fn line(&self, row: usize) -> String {
        self.rows[row].cells.iter().map(|c| c.ch).collect()
    }
}

/// A page's number, its title's band color, and how many subpages it
/// turns through.
struct Info {
    number: u16,
    band: Color,
    subpages: usize,
}

const INDEX: u16 = 100;
const NEWS: u16 = 101;
const CLOCK: u16 = 102;
const SEA: u16 = 103;
const PAGES: [Info; 4] = [
    Info { number: INDEX, band: Color::Blue, subpages: 1 },
    Info { number: NEWS, band: Color::Red, subpages: 3 },
    Info { number: CLOCK, band: Color::Magenta, subpages: 1 },
    Info { number: SEA, band: Color::Cyan, subpages: 1 },
];
/// The Fastext row: each link's color, words and page.
const LINKS: [(Color, &str, u16); 4] = [(Color::Red, "News", NEWS), (Color::Green, "Clock", CLOCK), (Color::Yellow, "At sea", SEA), (Color::Cyan, "Index", INDEX)];

fn info(number: u16) -> Option<&'static Info> {
    PAGES.iter().find(|p| p.number == number)
}

/// What a page is drawn from besides its number: the demo's seconds, the
/// time, which subpage, and whether concealed words show and flashing ones
/// are lit.
#[derive(Debug, Clone, Copy)]
pub struct View {
    pub t: f32,
    pub now: Time,
    pub sub: usize,
    pub reveal: bool,
    pub flash: bool,
    /// The clock's colon, lit for the first half of each second.
    pub colon: bool,
}

/// Where a page goes on a screen: its first column in the double lines'
/// cells, its first row, and the rows of it shown, top to bottom.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub left: u16,
    pub top: u16,
    pub rows: Vec<usize>,
}

impl Layout {
    /// For a screen `cols` × `rows`: it needs 80 columns, which a double
    /// line shows 40 of, and 24 rows, without the spare one over the
    /// Fastext row where there's no 25th. None on a smaller screen.
    pub fn new(cols: u16, rows: u16) -> Option<Self> {
        let shown = cols / 2;
        if usize::from(shown) < COLS || rows < 24 {
            return None;
        }
        let page: Vec<usize> = (0..ROWS).filter(|&r| rows >= 25 || r != SPARE).collect();
        Some(Self { left: (shown - COLS as u16) / 2, top: (rows - page.len() as u16) / 2, rows: page })
    }

    /// The screen's cell (column, row) at the middle of a page's cell.
    pub fn cell_at(&self, col: usize, row: usize) -> (u16, u16) {
        let r = self.rows.iter().position(|&r| r == row).unwrap_or(row.min(self.rows.len() - 1));
        ((self.left + col as u16) * 2 + 1, self.top + r as u16)
    }

    /// The page's cell under a click on the screen's (`col`, `row`): a
    /// double line's cells are counted as the line shows them.
    pub fn page_at(&self, col: u16, row: u16) -> Option<(usize, usize)> {
        let r = *self.rows.get(usize::from(row.checked_sub(self.top)?))?;
        let c = usize::from(col.checked_sub(self.left)?);
        (c < COLS).then_some((c, r))
    }
}

/// A row as escapes: to its line, its size, and its cells, each color
/// written only where it changes.
fn row_escape(row: &Row, screen_row: u16, left: u16) -> String {
    let r = screen_row + 1;
    let mut out = format!("\x1b[{r};1H{}\x1b[{r};{}H\x1b[0m", row.size.escape(), left + 1);
    let (mut fg, mut bg): (Option<Color>, Option<Color>) = (None, None);
    for cell in &row.cells {
        if cell.ch != ' ' && fg != Some(cell.fg) {
            let c = cell.fg.rgb();
            out += &format!("\x1b[38;2;{};{};{}m", c >> 16, (c >> 8) & 0xff, c & 0xff);
            fg = Some(cell.fg);
        }
        if cell.bg != bg {
            match cell.bg {
                Some(b) => {
                    let c = b.rgb();
                    out += &format!("\x1b[48;2;{};{};{}m", c >> 16, (c >> 8) & 0xff, c & 0xff);
                }
                None => out += "\x1b[49m",
            }
            bg = cell.bg;
        }
        out.push(cell.ch);
    }
    out + "\x1b[0m"
}

/// What was last written, to write only what changed.
#[derive(Default)]
struct Screen {
    size: Option<(u16, u16)>,
    rows: Vec<String>,
}

impl Screen {
    /// What brings the screen from what it showed to `page`.
    fn write(&mut self, page: &Page, cols: u16, rows: u16) -> String {
        let mut out = String::new();
        if self.size != Some((cols, rows)) {
            self.size = Some((cols, rows));
            self.rows.clear();
            out += "\x1b[0m\x1b[2J";
            if Layout::new(cols, rows).is_none() {
                let text = "A bigger window, please: 80 × 24 at least · q quits";
                let col = usize::from(cols).saturating_sub(text.chars().count()) / 2;
                out += &format!("\x1b[{};{}H{text}", rows / 2 + 1, col + 1);
            }
        }
        let Some(layout) = Layout::new(cols, rows) else { return out };
        self.rows.resize(layout.rows.len(), String::new());
        for (i, &r) in layout.rows.iter().enumerate() {
            let line = row_escape(&page.rows[r], layout.top + i as u16, layout.left);
            if self.rows[i] != line {
                out += &line;
                self.rows[i] = line;
            }
        }
        out
    }
}

/// The header: the page asked for, the service and the page coming round,
/// the date and the time.
fn header(page: &mut Page, asked: &str, rolling: u16, searching: bool, now: Time) {
    page.text(0, 1, asked, Color::White, None);
    page.text(0, 8, "FLOW", Color::Cyan, None);
    page.text(0, 13, &format!("{rolling:03}"), if searching { Color::Green } else { Color::White }, None);
    let date = format!("{} {:2} {}", now.day_short(), now.day, now.month_short());
    page.text(0, 19, &date, Color::White, None);
    page.text(0, 31, &format!("{:02}:{:02}/{:02}", now.hour, now.minute, now.second), Color::Yellow, None);
}

/// The Fastext row's four links.
fn fastext(page: &mut Page) {
    for (i, (color, words, _)) in LINKS.iter().enumerate() {
        page.text(FASTEXT, i * 10 + 1, words, *color, None);
    }
}

/// A page's title on a band of its color, twice as tall.
fn title(page: &mut Page, band: Color, words: &str, ink: Color) {
    page.tall(2);
    page.band(2, band);
    page.text(2, 1, words, ink, Some(band));
}

/// `text` wrapped to `width` columns from (`col`, `row`), a line a row;
/// returns the row after it.
fn wrapped(page: &mut Page, row: usize, col: usize, width: usize, text: &str, ink: Color) -> usize {
    let mut row = row;
    let mut line = String::new();
    for word in text.split(' ') {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            page.text(row, col, &line, ink, None);
            row += 1;
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line += word;
    }
    if !line.is_empty() {
        page.text(row, col, &line, ink, None);
        row += 1;
    }
    row
}

/// A page's body, without its header and Fastext row.
pub fn body(number: u16, v: &View) -> Option<Page> {
    let mut page = Page::blank();
    match number {
        INDEX => index(&mut page, v),
        NEWS => news(&mut page, v),
        CLOCK => clock_page(&mut page, v),
        SEA => sea_page(&mut page, v),
        _ => return None,
    }
    Some(page)
}

fn index(page: &mut Page, v: &View) {
    // FLOW in big mosaics on a band of blue, over a wave of the liquid.
    let mut logo = Pixels::new(COLS, 6);
    logo.rect(0, 0, logo.width as i32, logo.height as i32, Some(Color::Blue));
    let x = (logo.width as i32 - mosaic::letters_width("FLOW", 2)) / 2;
    logo.letters(x, 1, "FLOW", 2, Color::Yellow);
    for px in 0..logo.width as i32 {
        let crest = 16.0 + 1.2 * (px as f32 * 0.21 - v.t * 2.4).sin();
        logo.rect(px, crest.round() as i32, 1, 2, Some(Color::Cyan));
    }
    page.picture(1, 0, &logo.cells(&[Color::Blue]));
    page.tall(8);
    page.text(8, 2, "Teletext, live from the glass", Color::White, None);
    for (row, (words, number)) in [(11, ("News from the glass", NEWS)), (13, ("The time", CLOCK)), (15, ("At sea, in mosaics", SEA))] {
        page.text(row, 2, words, Color::White, None);
        let dots = 32 - words.chars().count();
        page.text(row, 3 + words.chars().count(), &".".repeat(dots), Color::Blue, None);
        page.text(row, 36, &number.to_string(), Color::Cyan, None);
    }
    // Flashing hides the letters, not their band.
    page.text(17, 1, if v.flash { " NEW " } else { "     " }, Color::White, Some(Color::Red));
    page.text(17, 7, "Type a page's number, or ← →", Color::Yellow, None);
    page.text(19, 2, "? reveals and h holds a page", Color::Green, None);
    page.text(21, 2, "Best in the School computer theme", Color::Green, None);
}

/// The news's subpages: a headline and its story, the last a quiz.
const STORIES: [(&str, &str); 3] = [
    (
        "Pictures ride the liquid",
        "Programs send pictures with kitty's graphics protocol, and Flow keeps them as cells of text, so they scroll and clear as words do. Each piece bends with the liquid under it. See flow-demo plasma.",
    ),
    (
        "Lines grow double",
        "This page is made of DEC's double lines: ESC # 6 makes one twice as wide, and ESC # 3 and ESC # 4 are the halves of one twice as tall. Flow draws the mosaics itself, so they join in any font.",
    ),
    ("Quiz of the day", "Which escape makes a line twice as wide, but no taller?"),
];

fn news(page: &mut Page, v: &View) {
    let sub = v.sub % STORIES.len();
    page.text(1, 35, &format!("{}/{}", sub + 1, STORIES.len()), Color::White, None);
    title(page, Color::Red, "FLOW NEWS", Color::Yellow);
    let (headline, story) = STORIES[sub];
    page.tall(5);
    page.text(5, 1, headline, Color::Cyan, None);
    let row = wrapped(page, 8, 1, 38, story, Color::White);
    if sub == 2 {
        page.text(row + 1, 1, "Press ? to reveal the answer.", Color::Green, None);
        // Concealed until revealed, as teletext's quizzes were.
        page.tall(row + 3);
        if v.reveal {
            page.text(row + 3, 4, "ESC # 6", Color::Yellow, None);
        }
    }
    page.text(21, 1, "Next subpage in a moment; h holds it", Color::Blue, None);
}

fn clock_page(page: &mut Page, v: &View) {
    title(page, Color::Magenta, "THE TIME", Color::White);
    let now = v.now;
    let time = format!("{:02}{}{:02}", now.hour, if v.colon { ':' } else { ' ' }, now.minute);
    let mut big = Pixels::new(COLS, 5);
    let x = (big.width as i32 - mosaic::letters_width("00:00", 2)) / 2;
    big.letters(x, 0, &time, 2, Color::Yellow);
    page.picture(5, 0, &big.cells(&[]));
    page.tall(11);
    let date = format!("{} {} {} {}", now.day_name(), now.day, now.month_name(), now.year);
    page.text(11, (COLS - date.len().min(COLS)) / 2, &date, Color::Cyan, None);
    // The minute's seconds as a bar across, on a track of separated ones.
    let fill = (usize::from(now.second) + 1) * 76 / 60;
    for i in 0..38 {
        let lit = fill.saturating_sub(i * 2).min(2);
        let cell = match lit {
            2 => Cell { ch: mosaic::sextant(63), fg: Color::Yellow, bg: None },
            1 => Cell { ch: mosaic::sextant(21), fg: Color::Yellow, bg: None },
            _ => Cell { ch: mosaic::separated(12), fg: Color::Blue, bg: None },
        };
        page.put(14, 1 + i, cell);
    }
    for (i, label) in [":00", ":15", ":30", ":45"].iter().enumerate() {
        page.text(15, 1 + i * 19 / 2, label, Color::White, None);
    }
    page.text(18, 2, "Your machine's own time, each second", Color::Green, None);
    page.text(19, 2, "as it comes. A new minute ripples", Color::Green, None);
    page.text(20, 2, "the glass.", Color::Green, None);
}

/// The boat at sea, drawn on `p` at `t`: the sun, clouds drifting, waves
/// rolling and the boat riding them. Returns where its bow is, in pixels.
pub fn sea(p: &mut Pixels, t: f32) -> (f32, f32) {
    let (w, h) = (p.width as i32, p.height as i32);
    p.disc(64.0, 7.0, 4.0, SQUASH, Some(Color::Yellow));
    for (i, (x, y)) in [(10.0, 6.0), (48.0, 3.0), (80.0, 9.0)].into_iter().enumerate() {
        let x = (x - t * (1.0 + i as f32 * 0.4)).rem_euclid(100.0) - 10.0;
        for (dx, dy, r) in [(0.0, 0.0, 3.0), (3.5, -1.5, 3.0), (7.0, 0.0, 2.5)] {
            p.disc(x + dx, y + dy, r, SQUASH * 0.7, Some(Color::White));
        }
    }
    let surface = |x: f32| 27.0 + 1.6 * (x * 0.22 - t * 1.8).sin() + 0.8 * (x * 0.09 + t * 0.7).sin();
    for x in 0..w {
        let top = surface(x as f32 + 0.5).round() as i32;
        p.rect(x, top, 1, h - top, Some(Color::Blue));
        p.set(x, top, Some(Color::Cyan));
        // Glints, rolling toward the shore.
        for y in top + 3..h {
            if (x as f32 * 0.5 + y as f32 * 1.3 - t * 3.0).sin() > 0.93 {
                p.set(x, y, Some(Color::Cyan));
            }
        }
    }
    let bx = 30.0 + 5.0 * (t * 0.13).sin();
    let by = surface(bx).round();
    let (bx, byi) = (bx.round() as i32, by as i32);
    for (dy, half) in [(-2, 9), (-1, 8), (0, 6)] {
        p.rect(bx - half, byi + dy, half * 2 + 1, 1, Some(Color::Red));
    }
    p.rect(bx, byi - 15, 1, 13, Some(Color::White));
    for k in 0..10 {
        // The mainsail aft of the mast, the jib before it.
        p.rect(bx + 1, byi - 14 + k, 1 + k * 7 / 10, 1, Some(Color::White));
        if k > 2 {
            p.rect(bx - 1 - (k - 3) * 5 / 10, byi - 14 + k, 1 + (k - 3) * 5 / 10, 1, Some(Color::White));
        }
    }
    p.set(bx, byi - 16, Some(Color::Red));
    ((bx + 9) as f32, by)
}

/// The scene's rows on the page, its foot in separated mosaics.
const SEA_ROWS: (usize, usize) = (5, 15);

fn sea_page(page: &mut Page, v: &View) {
    title(page, Color::Cyan, "AT SEA, IN MOSAICS", Color::Black);
    let mut p = Pixels::new(COLS, SEA_ROWS.1);
    sea(&mut p, v.t);
    let mut cells = p.cells(&[Color::Blue]);
    if let Some(foot) = cells.last_mut() {
        for cell in foot.iter_mut() {
            *cell = cell.separated();
        }
    }
    page.picture(SEA_ROWS.0, 0, &cells);
    page.text(21, 1, "Waves in sextants, the sea's foot in", Color::Green, None);
    page.text(22, 1, "separated ones, the sky the glass", Color::Green, None);
}

/// Teletext's own clock: the demo's seconds, the page shown and the one
/// being found, and what was last written.
pub struct Teletext {
    t: f32,
    shown: u16,
    /// Its subpage, and when that last turned.
    sub: usize,
    turned: f32,
    held: bool,
    reveal: bool,
    /// The digits typed toward a page's number.
    typed: Vec<u8>,
    /// The page asked for and when it comes round.
    wanted: Option<(u16, f32)>,
    /// The second shown, and when it began.
    second: Option<(u8, f32)>,
    minute: Option<u8>,
    /// Where the boat's bow was, and when it last splashed.
    bow: Option<f32>,
    splashed: f32,
    /// Effects, in the page's cells, and sounds, to go out with the frame.
    effects: Vec<Effect>,
    sounds: Vec<(Sound, (usize, usize))>,
    screen: Screen,
}

impl Teletext {
    pub fn new() -> Self {
        Self {
            t: 0.0,
            shown: INDEX,
            sub: 0,
            turned: 0.0,
            held: false,
            reveal: false,
            typed: Vec::new(),
            wanted: None,
            second: None,
            minute: None,
            bow: None,
            splashed: 0.0,
            effects: Vec::new(),
            sounds: Vec::new(),
            screen: Screen::default(),
        }
    }

    /// Goes to page `number` once it comes round, `after` seconds on.
    fn ask(&mut self, number: u16, after: f32) {
        self.typed.clear();
        self.wanted = Some((number, self.t + after));
    }

    /// The next page there is after the one shown, or before it.
    fn step(&self, by: i32) -> u16 {
        let i = PAGES.iter().position(|p| p.number == self.shown).unwrap_or(0) as i32;
        PAGES[(i + by).rem_euclid(PAGES.len() as i32) as usize].number
    }

    /// The whole page as it shows at `now`, moving on by `dt` first.
    pub fn page_at(&mut self, dt: f32, now: Time) -> Page {
        self.t += dt;
        let t = self.t;
        if let Some((number, at)) = self.wanted {
            if t >= at && info(number).is_some() {
                self.wanted = None;
                self.shown = number;
                self.sub = 0;
                self.turned = t;
                self.reveal = false;
                let band = info(number).map_or(Color::Blue, |i| i.band);
                self.effects.push(Effect::burst((20, 3)).color(band.rgb()).size(6.0).strength(0.45).quiet());
                self.effects.push(Effect::ripple((20, 3)).size(10.0).strength(0.4));
            }
        }
        let subpages = info(self.shown).map_or(1, |i| i.subpages);
        if !self.held && subpages > 1 && t - self.turned >= SUBPAGE {
            self.sub = (self.sub + 1) % subpages;
            self.turned = t;
            self.reveal = false;
        }
        if self.second.is_none_or(|(s, _)| s != now.second) {
            self.second = Some((now.second, t));
        }
        if self.minute.is_some_and(|m| m != now.minute) && self.shown == CLOCK {
            self.effects.push(Effect::ripple((20, 7)).size(16.0).strength(0.6).sound(Sound::Chime));
        }
        self.minute = Some(now.minute);
        let since = self.second.map_or(0.0, |(_, at)| t - at);
        let view = View { t, now, sub: self.sub, reveal: self.reveal, flash: t.fract() < FLASH_ON, colon: since < 0.5 };
        let mut page = body(self.shown, &view).unwrap_or_else(Page::blank);
        if self.shown == SEA {
            self.splash(&view);
        }
        let asked = match (self.held, self.typed.len(), self.wanted) {
            (true, _, _) => "HOLD".to_string(),
            (_, 0, Some((n, _))) => format!("P{n}"),
            (_, 0, None) => format!("P{}", self.shown),
            (_, _, _) => format!("P{:-<3}", self.typed.iter().map(|d| char::from(b'0' + d)).collect::<String>()),
        };
        let searching = self.wanted.is_some() || !self.typed.is_empty();
        let rolling = if searching { 100 + (t * 37.0) as u16 % 800 } else { self.shown };
        header(&mut page, &asked, rolling, searching, now);
        fastext(&mut page);
        page
    }

    /// The bow splashes the liquid each time it comes down into a wave,
    /// at most every two seconds.
    fn splash(&mut self, v: &View) {
        let mut p = Pixels::new(COLS, SEA_ROWS.1);
        let (x, y) = sea(&mut p, v.t);
        let was = self.bow.replace(y);
        let rising = was.is_some_and(|was| y < was);
        if rising && was.is_some_and(|was| was >= 28.0) && v.t - self.splashed > 2.0 {
            self.splashed = v.t;
            let at = ((x / 2.0) as u16, SEA_ROWS.0 as u16 + (y / 3.0) as u16);
            self.effects.push(Effect::burst(at).color(Color::Cyan.rgb()).size(3.0).strength(0.35));
        }
    }

    /// [`TextDemo::frame`] at `now`.
    pub fn frame_at(&mut self, dt: f32, cols: u16, rows: u16, now: Time) -> Written {
        let page = self.page_at(dt, now);
        let text = self.screen.write(&page, cols, rows);
        let (mut glass, mut heard) = (Vec::new(), Vec::new());
        if let Some(layout) = Layout::new(cols, rows) {
            for e in self.effects.drain(..) {
                let at = layout.cell_at(usize::from(e.at.0), usize::from(e.at.1));
                // Sizes are in the page's columns, which are two of the screen's.
                glass.push(Effect { at, size: e.size.map(|s| s * 2.0), ..e }.escape());
            }
            for (sound, (col, row)) in self.sounds.drain(..) {
                heard.push(sound.escape(Some(layout.cell_at(col, row)), Some(0.35)));
            }
        }
        self.effects.clear();
        self.sounds.clear();
        Written { text, glass, sounds: heard, ..Written::default() }
    }
}

impl Default for Teletext {
    fn default() -> Self {
        Self::new()
    }
}

impl TextDemo for Teletext {
    fn interval(&self) -> Duration {
        INTERVAL
    }

    fn frame(&mut self, dt: f32, size: term::Size) -> Written {
        self.frame_at(dt, size.cols, size.rows, clock::now())
    }

    fn key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char(c @ '0'..='9') => {
                let d = c as u8 - b'0';
                // Pages run from 100 to 899.
                if self.typed.is_empty() && !(1..=8).contains(&d) {
                    return;
                }
                self.typed.push(d);
                self.sounds.push((Sound::Click, (3, 0)));
                if self.typed.len() == 3 {
                    let n = self.typed.iter().fold(0u16, |n, &d| n * 10 + u16::from(d));
                    self.ask(n, SEARCH);
                }
            }
            KeyCode::Left => self.ask(self.step(-1), STEP),
            KeyCode::Right => self.ask(self.step(1), STEP),
            KeyCode::Char('h') => self.held = !self.held,
            KeyCode::Char('?') => self.reveal = !self.reveal,
            KeyCode::Char(c) => {
                if let Some((_, _, page)) = LINKS.iter().find(|(color, _, _)| fastext_key(*color) == Some(c)) {
                    self.ask(*page, SEARCH);
                }
            }
            _ => {}
        }
    }

    fn click(&mut self, col: u16, row: u16) {
        let Some((cols, rows)) = self.screen.size else { return };
        let Some((c, r)) = Layout::new(cols, rows).and_then(|l| l.page_at(col, row)) else { return };
        let page = match r {
            FASTEXT => LINKS.get(c / 10).map(|l| l.2),
            11 | 13 | 15 if self.shown == INDEX => Some([NEWS, CLOCK, SEA][(r - 11) / 2]),
            _ => None,
        };
        if let Some(page) = page {
            self.ask(page, SEARCH);
        }
    }
}

/// The key for a Fastext link: its color's first letter.
fn fastext_key(color: Color) -> Option<char> {
    match color {
        Color::Red => Some('r'),
        Color::Green => Some('g'),
        Color::Yellow => Some('y'),
        Color::Cyan => Some('c'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flow_sdk::fx::Kind;

    const NOON: Time = Time { year: 2026, month: 10, day: 31, weekday: 6, hour: 12, minute: 34, second: 56 };

    fn view(t: f32) -> View {
        View { t, now: NOON, sub: 0, reveal: false, flash: true, colon: true }
    }

    #[test]
    fn every_page_fits_forty_columns_and_its_tall_lines_come_in_halves() {
        for p in &PAGES {
            for sub in 0..p.subpages {
                let page = body(p.number, &View { sub, ..view(3.0) }).expect("a page");
                assert_eq!(page.rows.len(), ROWS);
                for (i, row) in page.rows.iter().enumerate() {
                    match row.size {
                        Size::Top => assert!(page.rows[i + 1].size == Size::Bottom && page.rows[i + 1].cells == row.cells, "page {} row {i}", p.number),
                        Size::Bottom => assert_eq!(page.rows[i - 1].size, Size::Top),
                        Size::Wide => {}
                    }
                }
                assert!(page.line(SPARE).trim().is_empty(), "the spare row stays blank on page {}", p.number);
            }
        }
        assert!(body(999, &view(0.0)).is_none());
    }

    #[test]
    fn the_header_shows_the_page_the_date_and_the_time() {
        let mut tt = Teletext::new();
        let page = tt.page_at(0.1, NOON);
        assert_eq!(page.line(0), " P100   FLOW 100   Sat 31 Oct  12:34/56 ");
        assert_eq!(page.rows[0].cells[31].fg, Color::Yellow);
        assert!(page.line(FASTEXT).starts_with(" News      Clock     At sea    Index"));
    }

    #[test]
    fn a_typed_page_is_found_after_the_header_counts_round() {
        let mut tt = Teletext::new();
        tt.key(KeyCode::Char('9'));
        assert!(tt.typed.is_empty(), "no page begins with 9");
        tt.key(KeyCode::Char('1'));
        tt.key(KeyCode::Char('0'));
        assert!(tt.page_at(0.1, NOON).line(0).starts_with(" P10-"));
        tt.key(KeyCode::Char('2'));
        let searching = tt.page_at(0.1, NOON);
        assert!(searching.line(0).starts_with(" P102"));
        assert_eq!(searching.rows[0].cells[13].fg, Color::Green, "the count rolls");
        assert_eq!(tt.shown, INDEX, "the old page stays till the new one comes");
        let found = tt.page_at(SEARCH, NOON);
        assert_eq!(tt.shown, CLOCK);
        assert_eq!(&found.line(0)[8..16], "FLOW 102");
        assert!(tt.effects.iter().any(|e| e.kind == Kind::Burst && e.color == Some(Color::Magenta.rgb())), "it splashes in its band's color");
        // A page there isn't is looked for and never comes.
        for d in ['5', '5', '5'] {
            tt.key(KeyCode::Char(d));
        }
        tt.page_at(60.0, NOON);
        assert_eq!((tt.shown, tt.wanted.map(|w| w.0)), (CLOCK, Some(555)));
    }

    #[test]
    fn the_news_turns_over_unless_held_and_its_answer_shows_when_revealed() {
        let mut tt = Teletext::new();
        tt.ask(NEWS, 0.0);
        tt.page_at(0.0, NOON);
        assert_eq!((tt.shown, tt.sub), (NEWS, 0));
        tt.page_at(SUBPAGE, NOON);
        assert_eq!(tt.sub, 1);
        tt.key(KeyCode::Char('h'));
        assert!(tt.page_at(SUBPAGE * 3.0, NOON).line(0).starts_with(" HOLD"));
        assert_eq!(tt.sub, 1, "held");
        tt.key(KeyCode::Char('h'));
        let quiz = tt.page_at(0.1, NOON);
        assert_eq!(tt.sub, 2);
        assert!(!(0..ROWS).any(|r| quiz.line(r).contains("ESC # 6")), "concealed");
        tt.key(KeyCode::Char('?'));
        let revealed = tt.page_at(0.1, NOON);
        let row = (0..ROWS).find(|&r| revealed.line(r).contains("ESC # 6")).expect("revealed");
        assert_eq!(revealed.rows[row].size, Size::Top);
    }

    #[test]
    fn the_clock_shows_the_time_in_mosaics_and_ripples_each_minute() {
        let mut tt = Teletext::new();
        tt.ask(CLOCK, 0.0);
        let page = tt.page_at(0.0, NOON);
        assert!(page.line(5).chars().any(|c| ('\u{1FB00}'..='\u{1FB3B}').contains(&c) || c == '█'), "big digits");
        assert!(page.line(11).contains("Saturday 31 October 2026"));
        assert_eq!(page.rows[11].size, Size::Top);
        // 56 seconds in: the bar is nearly full, the rest track.
        assert_eq!(page.rows[14].cells[1].ch, '█');
        assert_eq!(page.rows[14].cells[38].ch, mosaic::separated(12));
        tt.effects.clear();
        tt.page_at(1.0, Time { minute: 35, second: 0, ..NOON });
        assert!(tt.effects.iter().any(|e| e.kind == Kind::Ripple));
    }

    #[test]
    fn the_boat_rides_the_waves_and_its_bow_splashes_now_and_then() {
        let mut p = Pixels::new(COLS, SEA_ROWS.1);
        let (x, y) = sea(&mut p, 1.0);
        assert!((20.0..60.0).contains(&x) && (20.0..35.0).contains(&y));
        assert_eq!(p.get(x as i32 - 9, y as i32 - 2), Some(Color::Red), "the hull at the waterline");
        assert!(matches!(p.get(2, 40), Some(Color::Blue | Color::Cyan)), "the sea below");
        let mut tt = Teletext::new();
        tt.ask(SEA, 0.0);
        let mut splashes = 0;
        for _ in 0..600 {
            tt.page_at(0.1, NOON);
            splashes += tt.effects.drain(..).filter(|e| e.kind == Kind::Burst && e.color == Some(Color::Cyan.rgb())).count();
        }
        assert!((3..=30).contains(&splashes), "{splashes} splashes in a minute");
    }

    #[test]
    fn a_page_is_laid_out_on_double_lines_and_written_only_where_it_changed() {
        assert_eq!(Layout::new(79, 30), None);
        assert_eq!(Layout::new(80, 23), None);
        let small = Layout::new(80, 24).expect("fits");
        assert_eq!((small.left, small.top, small.rows.len()), (0, 0, 24));
        assert!(!small.rows.contains(&SPARE));
        let big = Layout::new(120, 30).expect("fits");
        assert_eq!((big.left, big.top), (10, 2));
        assert_eq!(big.cell_at(0, 0), (21, 2));
        assert_eq!(big.page_at(10, 2), Some((0, 0)));
        assert_eq!(big.page_at(9, 2), None);

        let mut tt = Teletext::new();
        let first = tt.frame_at(0.1, 120, 30, NOON).text;
        assert!(first.starts_with("\x1b[0m\x1b[2J"));
        assert_eq!(first.matches("\x1b#6").count() + first.matches("\x1b#3").count() + first.matches("\x1b#4").count(), ROWS);
        assert_eq!(first.matches("\x1b#3").count(), first.matches("\x1b#4").count());
        // Nothing is written past the half of the screen a double line
        // shows: no row starts further in than leaves it room.
        let moves: Vec<u16> = first
            .split("\x1b[")
            .skip(1)
            .filter_map(|s| {
                let end = s.find(|c: char| !c.is_ascii_digit() && c != ';')?;
                let (_, col) = s[..end].split_once(';').filter(|_| s[end..].starts_with('H'))?;
                col.parse().ok()
            })
            .collect();
        assert_eq!(moves.len(), ROWS * 2);
        assert!(moves.iter().all(|&c| c == 1 || c == 11), "{moves:?}");
        let again = tt.frame_at(0.01, 120, 30, NOON).text;
        assert!(again.len() < first.len() / 4, "only the rows that changed");
        assert!(!again.contains("\x1b[2J"));
        assert!(tt.frame_at(0.01, 120, 31, NOON).text.starts_with("\x1b[0m\x1b[2J"), "all of it after a resize");
        assert!(tt.frame_at(0.01, 60, 20, NOON).text.contains("A bigger window"));
    }

    #[test]
    fn digits_click_and_a_found_page_splashes_where_it_shows() {
        let mut tt = Teletext::new();
        tt.frame_at(0.1, 80, 25, NOON);
        tt.key(KeyCode::Char('1'));
        let typed = tt.frame_at(0.1, 80, 25, NOON);
        assert_eq!(typed.sounds, vec!["\x1b]7731;fx=sound;sound=click;at=7,0;strength=0.35\x07".to_string()]);
        assert!(typed.glass.is_empty(), "a sound alone isn't the liquid's");
        tt.key(KeyCode::Char('0'));
        tt.key(KeyCode::Char('3'));
        tt.frame_at(0.1, 80, 25, NOON);
        let found = tt.frame_at(SEARCH, 80, 25, NOON);
        assert!(found.glass.iter().any(|g| g.starts_with("\x1b]7731;fx=burst;at=41,3;color=00ffff;")), "{:?}", found.glass);
        // A click on the Fastext row's first link goes to the news.
        tt.click(3, 24);
        assert_eq!(tt.wanted.map(|w| w.0), Some(NEWS));
    }
}
