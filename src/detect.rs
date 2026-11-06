//! What the terminal can do, found by asking it: whether it shows kitty
//! pictures (kitty's query) and Ratty models (`ESC _ ratty;g;s ESC \`), a
//! cell's size in whole pixels (`CSI 16 t`), the text area's (`CSI 14 t`),
//! which terminal it is (XTVERSION) and its background color (OSC 11), with
//! DA1 last, which every terminal answers, so the answers end there however
//! few the others were. The
//! questions go out before anything reads the keys (crossterm would take
//! the answers for keys), with raw mode on (or the terminal echoes them),
//! and a terminal that answers nothing is given a second at most.
//!
//! A terminal that doesn't know a question ignores it, so a program asks
//! anywhere and falls back quietly: no pictures means letters, no models
//! means none, no cell size means the window's pixels or none, no
//! background means a program can't tell light from dark. On Windows
//! the console layer between Flow and the program (ConPTY) passes neither
//! questions nor answers, so they go down Flow's pipe for images
//! ([`crate::pipe`]) where Flow named one, and the pictures go that way too.

use std::io::{self, Write};
use std::time::Duration;

use crate::pipe::{Pipe, DA1};
use crate::screen::Rgb;
use crate::term::Size;

/// XTVERSION: which terminal this is. Flow answers `ESC P >|Flow <version> ESC \`.
pub const XTVERSION: &[u8] = b"\x1b[>0q";
/// A cell's size in whole pixels: `ESC [ 6 ; <height> ; <width> t`.
pub const CELL_SIZE: &[u8] = b"\x1b[16t";
/// The text area's size in pixels: `ESC [ 4 ; <height> ; <width> t`.
pub const AREA_SIZE: &[u8] = b"\x1b[14t";
/// OSC 11, the terminal's background color: `ESC ] 11 ; rgb:<r>/<g>/<b>`
/// and the question's own ending, each part one to four hex digits. Flow
/// answers with Settings' Background, as most terminals answer with
/// theirs.
pub const BACKGROUND: &[u8] = b"\x1b]11;?\x07";

/// Which questions to ask, and how long to wait for a terminal that
/// answers none of them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Query {
    /// Kitty's query: does it show pictures?
    pub pictures: bool,
    /// Ratty's `s`: does it show 3D models?
    pub models: bool,
    /// `CSI 16 t`: a cell's whole pixels.
    pub cell: bool,
    /// `CSI 14 t`: the text area's pixels.
    pub area: bool,
    /// XTVERSION: which terminal it is.
    pub version: bool,
    /// OSC 11: its background color.
    pub background: bool,
    /// On Windows, ask down Flow's pipe for images (and keep it for the
    /// pictures) where Flow named one.
    pub pipe: bool,
    /// How long to wait for a terminal that answers nothing at all, not
    /// even DA1.
    pub wait: Duration,
}

impl Default for Query {
    fn default() -> Self {
        Self::new()
    }
}

impl Query {
    /// DA1 alone, waiting a second at most, through the pipe on Windows.
    pub fn new() -> Self {
        Self { pictures: false, models: false, cell: false, area: false, version: false, background: false, pipe: true, wait: Duration::from_secs(1) }
    }

    /// Every question.
    pub fn everything() -> Self {
        Self { pictures: true, models: true, cell: true, area: true, version: true, background: true, ..Self::new() }
    }

    /// Kitty's query: does it show pictures?
    pub fn pictures(self) -> Self {
        Self { pictures: true, ..self }
    }

    /// Ratty's `s`: does it show 3D models?
    pub fn models(self) -> Self {
        Self { models: true, ..self }
    }

    /// `CSI 16 t`: a cell's whole pixels.
    pub fn cell(self) -> Self {
        Self { cell: true, ..self }
    }

    /// `CSI 14 t`: the text area's pixels, which give a cell's to a
    /// fraction of a pixel.
    pub fn area(self) -> Self {
        Self { area: true, ..self }
    }

    /// XTVERSION: which terminal it is.
    pub fn version(self) -> Self {
        Self { version: true, ..self }
    }

    /// OSC 11: its background color, for a program that sets its own
    /// colors to suit it ([`Caps::light_background`]).
    pub fn background(self) -> Self {
        Self { background: true, ..self }
    }

    /// Asks nothing down Flow's pipe on Windows, for a program that sends
    /// nothing there.
    pub fn without_pipe(self) -> Self {
        Self { pipe: false, ..self }
    }

    /// Waiting `wait` for a terminal that answers nothing, in place of a
    /// second.
    pub fn wait(self, wait: Duration) -> Self {
        Self { wait, ..self }
    }

    /// The questions, DA1 last.
    pub fn bytes(&self) -> Vec<u8> {
        let mut q = Vec::new();
        if self.area {
            q.extend(AREA_SIZE);
        }
        if self.pictures {
            q.extend(crate::picture::QUERY.as_bytes());
        }
        if self.models {
            q.extend(crate::model::QUERY.as_bytes());
        }
        if self.cell {
            q.extend(CELL_SIZE);
        }
        if self.version {
            q.extend(XTVERSION);
        }
        if self.background {
            q.extend(BACKGROUND);
        }
        q.extend(DA1);
        q
    }

    /// Asks the terminal, or on Windows Flow's pipe. Raw mode has to be on
    /// ([`crate::term::enter`]), and nothing else may be reading the keys
    /// yet. Flow is also known by name ([`crate::in_flow`]) where its
    /// answers don't come.
    pub fn ask(&self) -> Terminal {
        let (mut caps, pipe) = if cfg!(unix) {
            (parse(&ask_terminal(&self.bytes(), self.wait)), None)
        } else {
            self.ask_pipe()
        };
        caps.flow |= crate::in_flow();
        Terminal { caps, pipe }
    }

    /// Down Flow's pipe, with the text area's size too: ConPTY reports no
    /// pixels to work a cell's size out from. Elsewhere, and in other
    /// terminals on Windows, nothing is asked.
    fn ask_pipe(&self) -> (Caps, Option<Pipe>) {
        let Some(mut pipe) = self.pipe.then(Pipe::open).flatten() else { return (Caps::default(), None) };
        let caps = parse(&pipe.ask(&Query { area: true, ..*self }.bytes()));
        if caps.pictures || caps.models {
            (Caps { bare: caps.pictures, ..caps }, Some(pipe))
        } else {
            (caps, None)
        }
    }
}

/// What the terminal said it can do.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Caps {
    /// It shows pictures sent with the kitty graphics protocol.
    pub pictures: bool,
    /// It shows models placed with the Ratty graphics protocol.
    pub models: bool,
    /// A cell's size in whole pixels, width and height (`CSI 16 t`): the
    /// pixels a model's picture has per cell.
    pub cell: Option<(u16, u16)>,
    /// The text area's size in pixels, width and height (`CSI 14 t`).
    pub area: Option<(u32, u32)>,
    /// XTVERSION's answer: `Flow 0.1.24`, say.
    pub version: Option<String>,
    /// The background color OSC 11 gave, through Flow's pipe on Windows
    /// (since Flow 0.1.28; an older Flow there says none, as a terminal
    /// that doesn't say).
    pub background: Option<Rgb>,
    /// It's Flow, by XTVERSION or by name.
    pub flow: bool,
    /// Pictures go down Flow's pipe and the placeholder cells that show
    /// them carry only the image's id ([`crate::picture::bare_row`]):
    /// ConPTY gives each combining mark a column of its own and doesn't
    /// know the underline color, so a cell's row and column marks and its
    /// placement's color would garble the screen. Flow then takes a cell's
    /// column from the one to its left and its row as the first.
    pub bare: bool,
}

impl Caps {
    /// A cell's size in pixels for a terminal of `size`: from the text
    /// area's, to a fraction of a pixel, or the window's pixels over its
    /// cells, or the whole pixels `CSI 16 t` gave; none if nothing told.
    pub fn cell_size(&self, size: &Size) -> Option<(f32, f32)> {
        let area = self.area.filter(|_| size.cols > 0 && size.rows > 0).map(|(w, h)| (w as f32 / size.cols as f32, h as f32 / size.rows as f32));
        area.or(size.cell).or(self.cell.map(|(w, h)| (w as f32, h as f32)))
    }

    /// Whether the terminal's background is light ([`is_light`]); none if
    /// it didn't say what it is.
    pub fn light_background(&self) -> Option<bool> {
        self.background.map(is_light)
    }
}

/// Whether `color` is light: past the middle of the eye's brightness
/// (relative luminance over 0.179, where black and white letters read
/// equally well on it), so dark letters suit it better than light ones.
pub fn is_light(color: Rgb) -> bool {
    let linear = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let [r, g, b] = color;
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b) > 0.179
}

/// OSC 11's color in `text`: `rgb:` and three parts of one to four hex
/// digits each, each scaled from its own number of digits.
fn background(text: &str) -> Option<Rgb> {
    let rest = text.split("\x1b]11;").nth(1)?;
    let end = rest.find(['\x07', '\x1b'])?;
    let spec = rest[..end].strip_prefix("rgb:").or_else(|| rest[..end].strip_prefix("rgba:"))?;
    let mut parts = spec.split('/').map(|part| {
        let digits = part.len();
        let value = u32::from_str_radix(part, 16).ok().filter(|_| (1..=4).contains(&digits))?;
        let most = (1u32 << (4 * digits)) - 1;
        Some(((value * 255 + most / 2) / most) as u8)
    });
    Some([parts.next()??, parts.next()??, parts.next()??])
}

/// Reads the answers to [`Query::bytes`] in `reply`.
pub fn parse(reply: &[u8]) -> Caps {
    let text = String::from_utf8_lossy(reply);
    // `ESC [ <kind> ; <height> ; <width> t`, in pixels.
    let size = |kind: &str| {
        text.split(&format!("\x1b[{kind};")).nth(1).and_then(|rest| {
            let (h, w) = rest[..rest.find('t')?].split_once(';')?;
            let (w, h) = (w.parse::<u32>().ok()?, h.parse::<u32>().ok()?);
            (w > 0 && h > 0).then_some((w, h))
        })
    };
    let version = text.split("\x1bP>|").nth(1).and_then(|rest| rest.find("\x1b\\").map(|end| rest[..end].to_string()));
    Caps {
        pictures: text.contains("\x1b_Gi=31;OK"),
        models: text.contains("\x1b_ratty;g;s;"),
        cell: size("6").and_then(|(w, h)| Some((u16::try_from(w).ok()?, u16::try_from(h).ok()?))),
        area: size("4"),
        flow: version.as_deref().is_some_and(|v| v.starts_with("Flow")),
        version,
        background: background(&text),
        bare: false,
    }
}

/// The terminal as asked: what it can do, and where pictures go.
pub struct Terminal {
    /// What it said it can do.
    pub caps: Caps,
    pipe: Option<Pipe>,
}

impl Terminal {
    /// Asks the terminal `query`'s questions ([`Query::ask`]).
    pub fn detect(query: &Query) -> Self {
        query.ask()
    }

    /// A terminal that said it can do nothing, as one that answers nothing
    /// does: what a program shows when it isn't in a terminal, or in tests.
    pub fn plain() -> Self {
        Self { caps: Caps::default(), pipe: None }
    }

    /// Pictures and models go down Flow's pipe for images (Windows).
    pub fn piped(&self) -> bool {
        self.pipe.is_some()
    }

    /// Sends graphics commands, kitty's or Ratty's: down Flow's pipe where
    /// there is one, waiting until Flow has them, so they're in before the
    /// cells that show them; else to the terminal.
    pub fn send(&mut self, commands: &[u8]) -> io::Result<()> {
        if commands.is_empty() {
            return Ok(());
        }
        match self.pipe.as_mut() {
            Some(pipe) => pipe.send_and_wait(commands),
            None => write_out(commands),
        }
    }

    /// Sends effects and sounds ([`crate::fx`]): down Flow's pipe where
    /// there is one, else to the terminal. Send them after the screen that
    /// shows what set them off.
    pub fn send_effects(&mut self, escapes: &[u8]) -> io::Result<()> {
        if escapes.is_empty() {
            return Ok(());
        }
        match self.pipe.as_mut() {
            Some(pipe) => pipe.send(escapes),
            None => write_out(escapes),
        }
    }

    /// A cell's size in pixels now, after a resize say: asked down the pipe,
    /// or from the window's pixels, or as first told.
    pub fn cell_size(&mut self) -> Option<(f32, f32)> {
        let size = crate::term::size().ok()?;
        if let Some(pipe) = self.pipe.as_mut() {
            let area = parse(&pipe.ask(&[AREA_SIZE, DA1].concat())).area;
            if let Some(cell) = (Caps { area, ..Caps::default() }).cell_size(&Size { cell: None, ..size }) {
                return Some(cell);
            }
        }
        size.cell.or_else(|| self.caps.cell_size(&size))
    }
}

fn write_out(bytes: &[u8]) -> io::Result<()> {
    let mut out = io::stdout().lock();
    out.write_all(bytes)?;
    out.flush()
}

/// Writes `query` to the terminal and reads its answers until it answers
/// DA1, or `wait` passes.
#[cfg(unix)]
fn ask_terminal(query: &[u8], wait: Duration) -> Vec<u8> {
    ask_terminal_until(query, wait, crate::pipe::answered_da1)
}

/// Writes `query` to the terminal and reads its answers from the keys until
/// `done` says they're all there, or `wait` passes. Raw mode has to be on.
#[cfg(unix)]
fn ask_terminal_until(query: &[u8], wait: Duration, done: impl Fn(&[u8]) -> bool) -> Vec<u8> {
    let mut out = io::stdout();
    if out.write_all(query).and_then(|_| out.flush()).is_err() {
        return Vec::new();
    }
    read_answers(libc::STDIN_FILENO, wait, done)
}

/// Reads what comes in on `fd` until `done` says it's all there, or `wait`
/// passes.
#[cfg(unix)]
pub(crate) fn read_answers(fd: libc::c_int, wait: Duration, done: impl Fn(&[u8]) -> bool) -> Vec<u8> {
    use std::time::Instant;
    let deadline = Instant::now() + wait;
    let mut got = Vec::new();
    while !done(&got) {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let mut ask = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
        // SAFETY: one valid pollfd, and a buffer that outlives the read.
        let ready = unsafe { libc::poll(&mut ask, 1, left.as_millis().clamp(1, 1000) as i32) };
        if ready < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
            // A resize, say: keep waiting.
            continue;
        }
        // Nothing came, or the descriptor can't be waited on (macOS's poll
        // says so of some devices): a read would block.
        if ready <= 0 || ask.revents & libc::POLLIN == 0 {
            break;
        }
        let mut buf = [0u8; 1024];
        let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
        if n <= 0 {
            break;
        }
        got.extend_from_slice(&buf[..n as usize]);
    }
    got
}

#[cfg(not(unix))]
fn ask_terminal(_query: &[u8], _wait: Duration) -> Vec<u8> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flows_answers_say_it_shows_models_and_how_big_a_cell_is() {
        let reply = b"\x1b_ratty;g;s;v=1;fmt=obj|glb;update=1\x1b\\\x1b[6;38;18t\x1b[?62;4c";
        assert_eq!(parse(reply), Caps { models: true, cell: Some((18, 38)), ..Caps::default() });
        assert_eq!(parse(b"\x1b[?62;22c"), Caps::default(), "a terminal that only answers DA1");
    }

    #[test]
    fn every_answer_flow_gives_is_read() {
        let reply = b"\x1b[4;912;1323t\x1b_Gi=31;OK\x1b\\\x1b_ratty;g;s;v=1\x1b\\\x1b[6;38;16t\x1bP>|Flow 0.1.24\x1b\\\x1b[?62;22c";
        let caps = parse(reply);
        assert_eq!(caps, Caps { pictures: true, models: true, cell: Some((16, 38)), area: Some((1323, 912)), version: Some("Flow 0.1.24".into()), flow: true, background: None, bare: false });
        // The text area's size gives a cell's to a fraction of a pixel.
        assert_eq!(caps.cell_size(&Size { cols: 80, rows: 24, cell: None }), Some((1323.0 / 80.0, 38.0)));
        let kitty = parse(b"\x1bP>|kitty(0.40.1)\x1b\\\x1b[?62;c");
        assert_eq!((kitty.flow, kitty.version.as_deref()), (false, Some("kitty(0.40.1)")));
    }

    #[test]
    fn the_questions_go_in_one_order_with_da1_last() {
        assert_eq!(Query::new().models().cell().bytes(), b"\x1b_ratty;g;s\x1b\\\x1b[16t\x1b[c");
        assert_eq!(Query::new().pictures().cell().version().bytes(), b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[16t\x1b[>0q\x1b[c");
        assert!(Query::everything().bytes().starts_with(b"\x1b[14t") && Query::everything().bytes().ends_with(DA1));
        assert_eq!(Query::new().version().background().bytes(), b"\x1b[>0q\x1b]11;?\x07\x1b[c");
    }

    #[test]
    fn the_background_is_read_however_many_digits_it_comes_in() {
        // As Flow answers (alacritty's emulator), each byte doubled, ended
        // as the question was.
        let flow = parse(b"\x1b]11;rgb:d6d6/d2d2/c4c4\x07\x1b[?62;22c");
        assert_eq!((flow.background, flow.light_background()), (Some([0xd6, 0xd2, 0xc4]), Some(true)));
        assert_eq!(parse(b"\x1b]11;rgb:0808/0c0c/1616\x1b\\\x1b[?62;c").background, Some([8, 12, 22]));
        // Two digits, one, three, and xterm's four that aren't doubled.
        assert_eq!(parse(b"\x1b]11;rgb:ff/80/00\x07").background, Some([255, 128, 0]));
        assert_eq!(parse(b"\x1b]11;rgb:f/8/0\x07").background, Some([255, 136, 0]));
        assert_eq!(parse(b"\x1b]11;rgb:fff/000/800\x07").background, Some([255, 0, 128]));
        assert_eq!(parse(b"\x1b]11;rgb:ffff/0000/7fff\x07").background, Some([255, 0, 127]));
        // What doesn't read is no answer.
        for bad in [&b"\x1b]11;rgb:ff/80\x07"[..], b"\x1b]11;#ffffff\x07", b"\x1b]11;rgb:fffff/0/0\x07", b"\x1b]11;rgb:ff/80/00", b"\x1b[?62;22c"] {
            let caps = parse(bad);
            assert_eq!((caps.background, caps.light_background()), (None, None), "{:?}", String::from_utf8_lossy(bad));
        }
    }

    #[test]
    fn flows_themes_backgrounds_are_told_light_or_dark() {
        // The Workstation's white, the Teletype's paper and the Graphics
        // workstation's grey are light; Flow's glass, the green and amber
        // tubes and the Home computer's blue are dark.
        for light in [[0xff, 0xff, 0xff], [0xef, 0xe8, 0xd8], [0xd6, 0xd2, 0xc4], [0x80, 0x80, 0x80]] {
            assert!(is_light(light), "{light:?}");
        }
        for dark in [[0x08, 0x0c, 0x16], [0x02, 0x0b, 0x04], [0x12, 0x09, 0x00], [0x40, 0x31, 0x8d], [0x60, 0x60, 0x60]] {
            assert!(!is_light(dark), "{dark:?}");
        }
    }

    #[test]
    fn a_cell_is_measured_by_the_best_answer_there_is() {
        let size = Size { cols: 100, rows: 30, cell: Some((10.5, 21.0)) };
        let whole = Caps { cell: Some((10, 21)), ..Caps::default() };
        assert_eq!(whole.cell_size(&size), Some((10.5, 21.0)), "the window's pixels before whole ones");
        assert_eq!(whole.cell_size(&Size { cell: None, ..size }), Some((10.0, 21.0)));
        assert_eq!(Caps::default().cell_size(&Size { cell: None, ..size }), None);
    }
}
