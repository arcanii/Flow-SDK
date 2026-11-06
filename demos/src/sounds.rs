//! Sounds: Flow's sounds as a program asks for them, on a keyboard of
//! DECPS's two octaves (C5 to C7, as the VT520 had them) drawn in cells,
//! under a roll whose notes fall onto their keys. A tune plays from the
//! roll, each note sent as DECPS (`CSI <volume> ; <length> ; <note> , ~`)
//! as it reaches its key, which lights and splashes the liquid in the
//! note's color. The letters on the keys play them too, and the digits
//! play Flow's sounds by name (OSC 7731's `fx=sound`) from a row of pads,
//! each setting the liquid off its own way. ← and → pick a tune, space
//! stops it, ↑ and ↓ change the volume, z and x the letters' octave.
//!
//! Flow plays a program's notes one after another, each once the one
//! before has ended, so the demo keeps the same queue ([`Queue`]): a tune's
//! note goes out when it's due, a little shorter than its beat, and where
//! a key's note is still sounding then, the tune gives way rather than fall
//! behind. Flow takes named sounds and effects only so fast, which the demo
//! keeps under (the SDK's `fx::Budget`). An old machine's own sounds under
//! a theme (a tube's hum, a teletype's clatter) are Flow's alone, which no
//! program can ask for, and the screen says so.

use std::time::Duration;

use flow_sdk::fx::{self, Budget, Effect, Sound};
use flow_sdk::input::KeyCode;
use flow_sdk::term::Size;

use crate::cells::{self, Cell, Grid, Screen};
use crate::{TextDemo, Told, Written};

/// DECPS's notes: 1 is C5, a semitone each up to 25, C7.
const HIGHEST: u8 = 25;
const WHITE_KEYS: u16 = 15;
/// A white key's columns, the last a gap before the next.
const KEY_W: u16 = 4;
const KEYS_W: u16 = WHITE_KEYS * KEY_W - 1;
/// The keyboard's rows, the black keys in the top ones.
const KEY_ROWS: u16 = 5;
const BLACK_ROWS: u16 = 3;
/// A pad's columns, the last a gap, and how many to a row.
const PAD_W: u16 = 10;
const PADS_A_ROW: usize = 6;
const PAD_KEYS: [char; 11] = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-'];
/// Rows of words below the pads.
const WORDS: u16 = 5;
/// The rows besides the roll's: the title and a gap, the keyboard and the
/// row naming its Cs, a gap, two of pads, a gap and the words.
const FIXED: u16 = 2 + KEY_ROWS + 1 + 1 + 2 + 1 + WORDS;
const MIN_ROLL: u16 = 5;
const MAX_ROLL: u16 = 16;
pub const MIN_COLS: u16 = 72;
pub const MIN_ROWS: u16 = FIXED + MIN_ROLL;
/// Rows a second the roll's notes fall.
const FALL: f32 = 8.0;
/// A key's own note, in 32nds of a second, and how far ahead of now
/// Flow's notes may already run for a key's to wait behind them: a tune's
/// note a while, another key's hardly, so a held key's repeats don't pile
/// up seconds of notes.
const KEY_NOTE: u8 = 6;
const BEHIND_TUNE: f32 = 0.3;
const BEHIND_KEY: f32 = 0.05;
/// How much of its beat a tune's note sounds: detached, as a music box's
/// are, which leaves room for a key's between them.
const SOUNDS_FOR: f32 = 0.55;
/// How late a tune's note may start, behind a key's, before it gives way.
const LATE: f32 = 0.03;
/// Seconds before a tune begins, its first notes falling in, and between
/// one play of it and the next.
const LEAD: f32 = 1.5;
const PAUSE: f32 = 2.5;
/// Seconds a pad stays lit.
const PAD_GLOW: f32 = 0.5;
/// The volume DECPS plays at to begin with, of its 7.
const VOLUME: u8 = 5;

const WHITE: u32 = 0xe6e4ee;
const BLACK: u32 = 0x1c1a24;
const ON_WHITE: u32 = 0x6a6878;
const ON_BLACK: u32 = 0xa8a6b8;
const PAD: u32 = 0x2a2440;
const PAD_LIT: u32 = 0x8a7af0;
const TITLE: u32 = 0x7ce7ff;
const TEXT: u32 = 0x9aa0b4;
const HINT: u32 = 0x6a7084;
const GOLD: u32 = 0xffd040;

/// The letters that play notes, a semitone apart from a C up, white keys
/// on the home row and black ones above them, as a piano's lie.
const LETTERS: [char; 18] = ['a', 'w', 's', 'e', 'd', 'f', 't', 'g', 'y', 'h', 'u', 'j', 'k', 'o', 'l', 'p', ';', '\''];

/// A tune's note: DECPS's number (0 a rest), and its beat, in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Note {
    pub note: u8,
    pub start: f32,
    pub len: f32,
}

pub struct Tune {
    pub name: &'static str,
    pub notes: Vec<Note>,
    pub length: f32,
}

impl Tune {
    /// Notes written `E6/2 D#6/1 r/1`: each a note's name (`fx::note`'s)
    /// or `r` for a rest, and its length in `unit` seconds.
    pub fn parse(name: &'static str, unit: f32, text: &str) -> Result<Self, String> {
        let mut notes = Vec::new();
        let mut at = 0.0;
        for word in text.split_whitespace() {
            let (pitch, beats) = word.split_once('/').ok_or_else(|| format!("{word}: a note needs its length"))?;
            let note = fx::note(pitch).ok_or_else(|| format!("{word}: not a note DECPS plays"))?;
            let beats: f32 = beats.parse().map_err(|_| format!("{word}: its length isn't a number"))?;
            let len = beats * unit;
            notes.push(Note { note, start: at, len });
            at += len;
        }
        Ok(Self { name, notes, length: at })
    }
}

/// Beethoven's Ode to Joy, eighths of a quarter second.
const ODE: &str = "E6/2 E6/2 F6/2 G6/2 G6/2 F6/2 E6/2 D6/2 C6/2 C6/2 D6/2 E6/2 E6/3 D6/1 D6/4 \
                   E6/2 E6/2 F6/2 G6/2 G6/2 F6/2 E6/2 D6/2 C6/2 C6/2 D6/2 E6/2 D6/3 C6/1 C6/4 \
                   D6/2 D6/2 E6/2 C6/2 D6/2 E6/1 F6/1 E6/2 C6/2 D6/2 E6/1 F6/1 E6/2 D6/2 C6/2 D6/2 G5/4 \
                   E6/2 E6/2 F6/2 G6/2 G6/2 F6/2 E6/2 D6/2 C6/2 C6/2 D6/2 E6/2 D6/3 C6/1 C6/4";
/// The start of his Für Elise, in sixteenths.
const ELISE: &str = "E6/1 D#6/1 E6/1 D#6/1 E6/1 B5/1 D6/1 C6/1 A5/2 r/1 C5/1 E5/1 A5/1 B5/2 r/1 E5/1 G#5/1 B5/1 C6/2 r/1 E5/1 \
                     E6/1 D#6/1 E6/1 D#6/1 E6/1 B5/1 D6/1 C6/1 A5/2 r/1 C5/1 E5/1 A5/1 B5/2 r/1 E5/1 C6/1 B5/1 A5/4";
const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// DECPS's note `n` by name: `E6`.
pub fn name_of(n: u8) -> String {
    let s = usize::from(n.max(1) - 1);
    format!("{}{}", NAMES[s % 12], 5 + s / 12)
}

/// The tunes: two of Beethoven's, and every note DECPS has, up and down.
pub fn tunes() -> Vec<Tune> {
    let up: Vec<String> = (1..=HIGHEST).chain((1..HIGHEST).rev()).map(|n| format!("{}/1", name_of(n))).collect();
    [("Ode to Joy", 0.25, ODE.to_string()), ("Für Elise", 0.14, ELISE.to_string()), ("Two octaves", 0.11, up.join(" ") + " r/2")]
        .into_iter()
        .map(|(name, unit, text)| Tune::parse(name, unit, &text).expect("the tunes are written as DECPS plays them"))
        .collect()
}

/// Flow's notes as it queues them: each starts once the one before it has
/// ended, or at once if none is sounding. It also keeps whether the last
/// was a key's.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Queue {
    pub until: f32,
    pub key: bool,
}

impl Queue {
    /// A note `secs` long sent at `now`: when it starts.
    pub fn play(&mut self, now: f32, secs: f32, key: bool) -> f32 {
        let start = self.until.max(now);
        self.until = start + secs;
        self.key = key;
        start
    }
}

/// How long a tune's note of `beat` seconds sounds, in 32nds of a second:
/// some of its beat, and always short of it.
pub fn length(beat: f32) -> u8 {
    let slot = (beat * 32.0 + 1e-3).floor();
    (slot * SOUNDS_FOR).round().clamp(2.0, (slot - 1.0).max(1.0)) as u8
}

/// Where note `n`'s key is: whether it's black, and the white key it's on,
/// or right of.
fn key_of(n: u8) -> (bool, u16) {
    const PLACE: [(bool, u16); 12] = [(false, 0), (true, 0), (false, 1), (true, 1), (false, 2), (false, 3), (true, 3), (false, 4), (true, 4), (false, 5), (true, 5), (false, 6)];
    let s = n.clamp(1, HIGHEST) - 1;
    let (black, i) = PLACE[usize::from(s % 12)];
    (black, u16::from(s / 12) * 7 + i)
}

/// The columns note `n`'s key covers from the keyboard's left: a white
/// key's three, or a black key's three over the gap between two.
fn lane(n: u8) -> u16 {
    let (black, k) = key_of(n);
    k * KEY_W + if black { 2 } else { 0 }
}

/// The note whose key is at (`col`, `row`) of the keyboard, from its top
/// left: a black key over the white ones in the top rows.
fn note_at(col: u16, row: u16) -> Option<u8> {
    if row >= KEY_ROWS || col >= KEYS_W {
        return None;
    }
    let black = (1..=HIGHEST).find(|&n| key_of(n).0 && row < BLACK_ROWS && (lane(n)..lane(n) + 3).contains(&col));
    black.or_else(|| (1..=HIGHEST).find(|&n| !key_of(n).0 && key_of(n).1 == col / KEY_W))
}

/// A note's color: its place in the octave round the color wheel.
fn color_of(n: u8) -> u32 {
    let pc = f32::from((n.max(1) - 1) % 12);
    cells::hsv(pc / 12.0, 0.55, if key_of(n).0 { 0.8 } else { 1.0 })
}

/// Where everything goes on a screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub cols: u16,
    /// The keyboard's first column.
    pub left: u16,
    /// The title's row, the roll's rows (below it, and a gap), the
    /// keyboard's first row, the pads' and the words'.
    pub top: u16,
    pub roll: u16,
    pub keys: u16,
    pub pads: u16,
    pub words: u16,
}

impl Layout {
    /// For a screen `cols` × `rows`, None if it's under 72 × 22.
    pub fn new(cols: u16, rows: u16) -> Option<Self> {
        if cols < MIN_COLS || rows < MIN_ROWS {
            return None;
        }
        let roll = (rows - FIXED).min(MAX_ROLL);
        let top = (rows - FIXED - roll) / 2;
        let keys = top + 2 + roll;
        let pads = keys + KEY_ROWS + 2;
        Some(Self { cols, left: (cols - KEYS_W) / 2, top, roll, keys, pads, words: pads + 3 })
    }

    /// The cell at the top of note `n`'s key, where it splashes the liquid.
    pub fn key_cell(&self, n: u8) -> (u16, u16) {
        (self.left + lane(n) + 1, self.keys)
    }

    /// Pad `i`'s first column and its row: six on the first row, the
    /// rest centered below them.
    pub fn pad(&self, i: usize) -> (u16, u16) {
        let (row, j) = (i / PADS_A_ROW, i % PADS_A_ROW);
        let count = (if row == 0 { PADS_A_ROW } else { PAD_KEYS.len() - PADS_A_ROW }) as u16;
        ((self.cols - count * PAD_W) / 2 + j as u16 * PAD_W, self.pads + row as u16)
    }

    pub fn pad_at(&self, col: u16, row: u16) -> Option<usize> {
        (0..PAD_KEYS.len()).find(|&i| {
            let (c, r) = self.pad(i);
            r == row && (c..c + PAD_W - 1).contains(&col)
        })
    }
}

pub struct Sounds {
    t: f32,
    tunes: Vec<Tune>,
    tune: usize,
    playing: bool,
    /// Seconds into the tune; below 0 while it's about to begin.
    at: f32,
    /// The tune's next note to send.
    next: usize,
    volume: u8,
    /// The octave the letters start from: 0 at C5, 1 at C6.
    octave: u8,
    queue: Queue,
    /// Notes sounding, or waiting in Flow's queue: which, from when, till when.
    sounding: Vec<(u8, f32, f32)>,
    /// When each pad last played.
    pads: [Option<f32>; 11],
    budget: Budget,
    told: Told,
    /// What went out last, in words.
    sent: String,
    layout: Option<Layout>,
    /// To go out with the frame: effects, and sounds and notes.
    glass: Vec<String>,
    heard: Vec<String>,
    screen: Screen,
}

impl Sounds {
    pub fn new() -> Self {
        Self {
            t: 0.0,
            tunes: tunes(),
            tune: 0,
            playing: true,
            at: -LEAD,
            next: 0,
            volume: VOLUME,
            octave: 0,
            queue: Queue::default(),
            sounding: Vec::new(),
            pads: [None; 11],
            budget: Budget::new(),
            told: Told { flow: true, effects: true },
            sent: String::new(),
            layout: None,
            glass: Vec::new(),
            heard: Vec::new(),
            screen: Screen::default(),
        }
    }

    /// Plays note `n` for `len` 32nds of a second, as DECPS has it, and
    /// splashes the liquid at its key: a key's always, as far as the budget
    /// goes, and a tune's only while some of it is left for the keys.
    fn play_note(&mut self, n: u8, len: u8, by_key: bool) {
        let secs = f32::from(len) / 32.0;
        let start = self.queue.play(self.t, secs, by_key);
        let keep = if by_key { 0.0 } else { 3.0 };
        self.sounding.push((n, start, start + secs));
        self.heard.push(fx::notes(self.volume, len, &[n]));
        self.sent = format!("CSI {};{len};{n},~ (DECPS: {} for {len}/32 s at volume {} of 7)", self.volume, name_of(n), self.volume);
        if let Some(layout) = self.layout.filter(|_| self.told.effects) {
            if self.budget.take(self.t, keep) {
                self.glass.push(Effect::burst(layout.key_cell(n)).color(color_of(n)).size(1.5).strength(0.35).quiet().escape());
            }
        }
    }

    /// A key's note, unless Flow's queue already runs ahead of it.
    fn play_key(&mut self, n: u8) {
        let behind = if self.queue.key { BEHIND_KEY } else { BEHIND_TUNE };
        if (1..=HIGHEST).contains(&n) && self.queue.until - self.t <= behind {
            self.play_note(n, KEY_NOTE, true);
        }
    }

    /// Flow's sound `i` by name from its pad, and the liquid set off there.
    fn play_pad(&mut self, i: usize) {
        let Some(&sound) = Sound::ALL.get(i) else { return };
        self.pads[i] = Some(self.t);
        let at = self.layout.map(|l| {
            let (col, row) = l.pad(i);
            (col + PAD_W / 2, row)
        });
        // Flow would drop a sound past its allowance anyway.
        if !self.budget.take(self.t, 0.0) {
            return;
        }
        self.heard.push(sound.escape_with(at, Some(0.6), f32::from(self.volume) / 7.0));
        let place = at.map(|(c, r)| format!(" at={c},{r}")).unwrap_or_default();
        self.sent = format!("OSC 7731 fx=sound sound={}{place} (one of Flow's by name)", sound.name());
        // Its effect leaves one for the next pad's sound.
        if let (Some(at), Some(layout)) = (at, self.layout.filter(|_| self.told.effects)) {
            if self.budget.take(self.t, 1.0) {
                self.glass.push(pad_effect(sound, at, layout.top + 2).escape());
            }
        }
    }

    /// Moves the tune on by `dt`, sending each note as it comes due.
    fn play_tune(&mut self, dt: f32) {
        self.at += dt;
        while let Some(n) = self.tunes[self.tune].notes.get(self.next).copied() {
            if n.start > self.at {
                break;
            }
            self.next += 1;
            if n.note == 0 {
                continue;
            }
            // When it was due, by the demo's clock; a key's note still
            // sounding then has the tune give way, not fall behind.
            let due = self.t - (self.at - n.start);
            if self.queue.until > due + LATE {
                continue;
            }
            self.play_note(n.note, length(n.len), false);
        }
        let tune = &self.tunes[self.tune];
        if self.next >= tune.notes.len() && self.at >= tune.length {
            // Its end rings, and goes round again after a pause.
            if let Some(l) = self.layout {
                let middle = (l.left + KEYS_W / 2, l.keys);
                if self.budget.take(self.t, 2.0) {
                    self.heard.push(Sound::Chime.escape_with(Some(middle), Some(0.5), f32::from(self.volume) / 7.0));
                }
                if self.told.effects && self.budget.take(self.t, 2.0) {
                    self.glass.push(Effect::burst(middle).color(GOLD).size(12.0).strength(0.5).quiet().escape());
                }
            }
            self.at = -PAUSE;
            self.next = 0;
        }
    }

    fn pick(&mut self, tune: usize) {
        self.tune = tune % self.tunes.len();
        self.at = -LEAD;
        self.next = 0;
        self.playing = true;
    }

    /// Note `n`'s color while it sounds.
    fn lit(&self, n: u8) -> Option<u32> {
        self.sounding.iter().any(|&(m, start, end)| m == n && start <= self.t && self.t < end).then(|| color_of(n))
    }

    /// The screen at the demo's time.
    fn draw(&self, cols: u16, rows: u16) -> Grid {
        let mut g = Grid::new(cols, rows);
        let Some(l) = self.layout else {
            g.centered(rows / 2, &format!("A bigger window, please: {MIN_COLS} × {MIN_ROWS} at least · q quits"), TEXT, false);
            return g;
        };
        g.centered(l.top, "Flow's sounds, as a program asks for them", TITLE, true);
        self.draw_roll(&mut g, &l);
        // The white keys, then the black ones over them.
        for n in (1..=HIGHEST).filter(|&n| !key_of(n).0) {
            let (_, k) = key_of(n);
            g.fill(l.left + k * KEY_W, l.keys, KEY_W - 1, KEY_ROWS, Some(self.lit(n).unwrap_or(WHITE)));
        }
        for n in (1..=HIGHEST).filter(|&n| key_of(n).0) {
            g.fill(l.left + lane(n), l.keys, 3, BLACK_ROWS, Some(self.lit(n).map_or(BLACK, |c| cells::scale(c, 0.75))));
        }
        for (i, &letter) in LETTERS.iter().enumerate() {
            let n = 1 + 12 * self.octave + i as u8;
            if n > HIGHEST {
                break;
            }
            let (black, k) = key_of(n);
            let mut s = [0u8; 4];
            let letter = letter.encode_utf8(&mut s);
            if black {
                g.text(l.left + lane(n) + 1, l.keys + BLACK_ROWS - 1, letter, if self.lit(n).is_some() { BLACK } else { ON_BLACK }, false);
            } else {
                g.text(l.left + k * KEY_W + 1, l.keys + KEY_ROWS - 1, letter, if self.lit(n).is_some() { BLACK } else { ON_WHITE }, false);
            }
        }
        for (k, name) in [(0, "C5"), (7, "C6"), (14, "C7")] {
            g.text(l.left + k * KEY_W, l.keys + KEY_ROWS, name, HINT, false);
        }
        for (i, sound) in Sound::ALL.iter().enumerate() {
            let (col, row) = l.pad(i);
            let glow = self.pads[i].map_or(0.0, |at| 1.0 - (self.t - at) / PAD_GLOW).clamp(0.0, 1.0);
            g.fill(col, row, PAD_W - 1, 1, Some(cells::blend(PAD, PAD_LIT, glow)));
            g.text(col + 1, row, &format!("{} {}", PAD_KEYS[i], sound.name()), if glow > 0.5 { 0xffffff } else { TEXT }, false);
        }
        for (i, line) in self.words().iter().enumerate() {
            g.centered(l.words + i as u16, line, if i == 0 { TEXT } else { HINT }, i == 0);
        }
        g
    }

    /// The tune's notes still to come, falling toward their keys, a cell's
    /// half a step: the white keys' first, then the black's over them.
    fn draw_roll(&self, g: &mut Grid, l: &Layout) {
        let notes = &self.tunes[self.tune].notes;
        let halves = f32::from(l.roll) * 2.0;
        for black in [false, true] {
            for n in notes.iter().filter(|n| n.note != 0 && key_of(n.note).0 == black) {
                // Seconds until it reaches its key, and until its sound ends.
                let (from, to) = (n.start - self.at, n.start - self.at + n.len - 1.0 / 32.0);
                if to <= 0.0 || from * FALL * 2.0 >= halves {
                    continue;
                }
                let filled = |h: f32| (from..to).contains(&((h + 0.5) / (FALL * 2.0)));
                for r in 0..l.roll {
                    let (low, high) = (filled(f32::from(r) * 2.0), filled(f32::from(r) * 2.0 + 1.0));
                    let ch = match (low, high) {
                        (true, true) => '█',
                        (true, false) => '▄',
                        (false, true) => '▀',
                        (false, false) => continue,
                    };
                    let row = l.keys - 1 - r;
                    for c in 0..3 {
                        g.put(l.left + lane(n.note) + c, row, Cell::new(ch, color_of(n.note)));
                    }
                }
            }
        }
    }

    /// The rows of words below the pads.
    fn words(&self) -> [String; WORDS as usize] {
        let tune = self.tunes[self.tune].name;
        let space = if self.playing { "space stops it" } else { "space plays it" };
        let sent = match (self.sent.is_empty(), self.told.flow) {
            (true, _) => "Nothing sent yet".to_string(),
            (false, true) => format!("Sent: {}", self.sent),
            (false, false) => format!("Not sent: {}", self.sent),
        };
        let heard = if self.told.flow { "Heard with Sound on in Flow's Settings; notes play one after another" } else { "Not in Flow: the keys light, but these sounds are Flow's alone" };
        [
            format!("{tune} · {space} · ← → a tune · ↑ ↓ volume {}/7 · q quits", self.volume),
            sent,
            "Letters on the keys play notes (z x an octave), digits Flow's sounds".to_string(),
            heard.to_string(),
            "A theme's hum and clatter are Flow's own: no program can ask for them".to_string(),
        ]
    }

    /// [`TextDemo::frame`] for a screen `cols` × `rows`.
    pub fn frame_at(&mut self, dt: f32, cols: u16, rows: u16) -> Written {
        self.t += dt;
        self.layout = Layout::new(cols, rows);
        if self.playing {
            self.play_tune(dt);
        }
        self.sounding.retain(|&(_, _, end)| end > self.t);
        let text = self.screen.write(&self.draw(cols, rows));
        Written { text, glass: std::mem::take(&mut self.glass), sounds: std::mem::take(&mut self.heard), ..Written::default() }
    }
}

impl Default for Sounds {
    fn default() -> Self {
        Self::new()
    }
}

/// How the liquid moves for each of Flow's sounds played from its pad at
/// `at`, quietly, since the sound goes by name: a bolt reaches up to row
/// `sky`.
fn pad_effect(sound: Sound, at: (u16, u16), sky: u16) -> Effect {
    let e = match sound {
        Sound::Drop => Effect::ripple(at).size(3.0),
        Sound::Splash => Effect::burst(at).color(0x40a0ff).size(3.0),
        Sound::Thunder => Effect::bolt(at, (at.0, sky)).color(0xc0d0ff).strength(0.7),
        Sound::Whoosh => Effect::stir(at, (1.0, 0.0)).size(8.0),
        Sound::Chime => Effect::burst(at).color(GOLD).size(3.0),
        Sound::Low => Effect::ripple(at).size(7.0).strength(0.3),
        Sound::Ding => Effect::ripple(at).size(2.0).strength(0.6),
        Sound::Thud => Effect::stir(at, (0.0, 1.0)).size(3.0),
        Sound::Click => Effect::ripple(at).size(1.0).strength(0.3),
        Sound::Boing => Effect::burst(at).color(0xff60c0).size(2.5),
        Sound::Bell => Effect::ripple(at).size(4.0).strength(0.5),
    };
    e.quiet()
}

impl TextDemo for Sounds {
    fn told(&mut self, told: Told) {
        self.told = told;
    }

    fn interval(&self) -> Duration {
        crate::FRAME
    }

    fn frame(&mut self, dt: f32, size: Size) -> Written {
        self.frame_at(dt, size.cols, size.rows)
    }

    fn key(&mut self, key: KeyCode) {
        let n = self.tunes.len();
        match key {
            KeyCode::Char(' ') => self.playing = !self.playing,
            KeyCode::Left => self.pick(self.tune + n - 1),
            KeyCode::Right => self.pick(self.tune + 1),
            KeyCode::Up => self.volume = (self.volume + 1).min(7),
            KeyCode::Down => self.volume = self.volume.saturating_sub(1).max(1),
            KeyCode::Char('z') => self.octave = 0,
            KeyCode::Char('x') => self.octave = 1,
            KeyCode::Char(c) => {
                if let Some(i) = LETTERS.iter().position(|&l| l == c) {
                    self.play_key(1 + 12 * self.octave + i as u8);
                } else if let Some(i) = PAD_KEYS.iter().position(|&k| k == c) {
                    self.play_pad(i);
                }
            }
            _ => {}
        }
    }

    fn click(&mut self, col: u16, row: u16) {
        let Some(l) = self.layout else { return };
        let on_keys = col.checked_sub(l.left).zip(row.checked_sub(l.keys)).and_then(|(c, r)| note_at(c, r));
        if let Some(n) = on_keys {
            self.play_key(n);
        } else if let Some(i) = l.pad_at(col, row) {
            self.play_pad(i);
        } else if row == l.words {
            self.playing = !self.playing;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays `secs` of frames `dt` apart on an 80 × 24 screen, gathering
    /// what goes out.
    fn run(s: &mut Sounds, secs: f32, dt: f32) -> (Vec<String>, Vec<String>) {
        let (mut glass, mut heard) = (Vec::new(), Vec::new());
        for _ in 0..(secs / dt).round() as usize {
            let w = s.frame_at(dt, 80, 24);
            glass.extend(w.glass);
            heard.extend(w.sounds);
        }
        (glass, heard)
    }

    #[test]
    fn every_tune_is_in_decps_range_and_its_notes_follow_one_another() {
        let tunes = tunes();
        assert_eq!(tunes.len(), 3);
        for tune in &tunes {
            assert!(tune.notes.iter().all(|n| n.note <= HIGHEST && n.len > 0.05), "{}", tune.name);
            for w in tune.notes.windows(2) {
                assert!((w[1].start - w[0].start - w[0].len).abs() < 1e-4, "{}", tune.name);
            }
            assert!((5.0..40.0).contains(&tune.length), "{} is {} s", tune.name, tune.length);
        }
        let all: Vec<u8> = tunes[2].notes.iter().map(|n| n.note).filter(|&n| n > 0).collect();
        assert_eq!((all.len(), all[0], all[24], *all.last().unwrap()), (49, 1, 25, 1), "up two octaves and down again");
        assert_eq!((name_of(1), name_of(17), name_of(25)), ("C5".to_string(), "E6".to_string(), "C7".to_string()));
        assert!(Tune::parse("bad", 0.1, "C4/1").is_err(), "below DECPS's lowest");
        assert!(Tune::parse("bad", 0.1, "C5").is_err());
    }

    #[test]
    fn the_keys_lie_as_a_pianos_and_a_click_finds_each() {
        assert_eq!((1..=HIGHEST).filter(|&n| key_of(n).0).count(), 10);
        assert_eq!((1..=HIGHEST).filter(|&n| !key_of(n).0).count(), usize::from(WHITE_KEYS));
        assert_eq!((key_of(1), key_of(2), key_of(5), key_of(6), key_of(25)), ((false, 0), (true, 0), (false, 2), (false, 3), (false, 14)));
        for n in 1..=HIGHEST {
            let row = if key_of(n).0 { 0 } else { KEY_ROWS - 1 };
            assert_eq!(note_at(lane(n) + 1, row), Some(n), "note {n}");
        }
        assert_eq!(note_at(lane(2) + 1, KEY_ROWS - 1), Some(1), "below the black keys, the white key");
        assert_eq!(note_at(KEYS_W, 0), None);
        assert!(lane(25) + 3 <= KEYS_W, "the last key fits");
    }

    #[test]
    fn a_tune_sends_each_note_as_decps_as_it_reaches_its_key() {
        let mut s = Sounds::new();
        let (_, before) = run(&mut s, LEAD - 0.05, 0.01);
        assert!(before.is_empty(), "nothing until the first note falls onto its key");
        let (glass, heard) = run(&mut s, 0.1, 0.01);
        // E6 (17) on a beat of half a second, sounding for 9/32 of it.
        assert_eq!(heard, vec!["\x1b[5;9;17,~".to_string()]);
        assert!(glass[0].starts_with(&format!("\x1b]7731;fx=burst;at={},{};color=", s.layout.unwrap().key_cell(17).0, s.layout.unwrap().keys)));
        assert!(s.lit(17).is_some(), "its key lights while it sounds");
        // A tune's notes never run past their beats in Flow's queue.
        let (_, heard) = run(&mut s, 10.0, 0.016);
        assert!(heard.len() >= 18, "{}", heard.len());
        assert!(s.queue.until <= s.t + 0.5 + 1e-3);
        assert_eq!((length(0.25), length(0.5), length(1.0), length(0.14), length(0.11)), (4, 9, 18, 2, 2));
    }

    #[test]
    fn the_roll_shows_the_next_notes_falling_onto_their_keys() {
        let mut s = Sounds::new();
        s.frame_at(LEAD - 0.5, 80, 24);
        let l = s.layout.unwrap();
        let g = s.draw(80, 24);
        // Half a second off at 8 rows a second: four rows of glass between
        // it and its key.
        let col = l.left + lane(17) + 1;
        let rows: Vec<u16> = (l.top..l.keys).filter(|&r| g.get(col, r).is_some_and(|c| c.ch != ' ')).collect();
        assert_eq!(rows.last().copied(), Some(l.keys - 5), "{rows:?}");
        assert_eq!(g.get(col, l.keys - 5).unwrap().fg, color_of(17));
        assert_eq!(g.get(col, l.keys).unwrap().bg, Some(WHITE), "not lit yet");
    }

    #[test]
    fn a_keys_note_has_the_tune_give_way_rather_than_fall_behind() {
        let mut s = Sounds::new();
        run(&mut s, LEAD + 0.4, 0.01);
        // Just before the second note is due, a key: it plays at once, and
        // the tune's second gives way to it.
        s.key(KeyCode::Char('a'));
        assert_eq!(s.frame_at(0.001, 80, 24).sounds, vec!["\x1b[5;6;1,~".to_string()]);
        let (_, heard) = run(&mut s, 0.15, 0.01);
        assert!(heard.is_empty(), "{heard:?}");
        assert_eq!(s.sounding.last().map(|n| n.0), Some(1), "C5 is sounding");
        // And the third comes on its beat.
        let (_, heard) = run(&mut s, 0.5, 0.01);
        assert_eq!(heard, vec!["\x1b[5;9;18,~".to_string()], "F6, on time");
        // A key during a tune's note waits for it to end.
        let start = s.queue.until;
        s.key(KeyCode::Char('s'));
        assert_eq!(s.sounding.last().map(|n| (n.0, n.1)), Some((3, start)));
    }

    #[test]
    fn a_held_key_doesnt_pile_notes_up_ahead() {
        let mut s = Sounds::new();
        s.playing = false;
        s.frame_at(0.01, 80, 24);
        for _ in 0..30 {
            s.key(KeyCode::Char('h'));
        }
        let w = s.frame_at(0.01, 80, 24);
        assert_eq!(w.sounds, vec!["\x1b[5;6;10,~".to_string()], "A5, once");
        s.key(KeyCode::Char('x'));
        s.key(KeyCode::Up);
        run(&mut s, 0.3, 0.01);
        s.key(KeyCode::Char('h'));
        assert_eq!(s.frame_at(0.01, 80, 24).sounds, vec!["\x1b[6;6;22,~".to_string()], "an octave up, louder");
    }

    #[test]
    fn the_digits_play_flows_sounds_by_name_and_set_off_the_liquid() {
        let mut s = Sounds::new();
        s.playing = false;
        s.frame_at(0.01, 80, 24);
        let l = s.layout.unwrap();
        s.key(KeyCode::Char('3'));
        let w = s.frame_at(0.01, 80, 24);
        let (col, row) = l.pad(2);
        assert_eq!(w.sounds, vec![format!("\x1b]7731;fx=sound;sound=thunder;at={},{row};strength=0.60;volume=0.71\x07", col + PAD_W / 2)]);
        assert_eq!(w.glass.len(), 1);
        assert!(w.glass[0].starts_with(&format!("\x1b]7731;fx=bolt;at={},{row};to={},{};", col + PAD_W / 2, col + PAD_W / 2, l.top + 2)) && w.glass[0].contains("sound=none"), "{:?}", w.glass);
        assert!(s.draw(80, 24).get(col, row).unwrap().bg != Some(PAD), "its pad lights");
        // A click on a pad plays it too.
        run(&mut s, 2.0, 0.1);
        let (col, row) = l.pad(10);
        s.click(col + 2, row);
        assert!(s.frame_at(0.01, 80, 24).sounds[0].contains("sound=bell"));
    }

    #[test]
    fn it_keeps_under_what_flow_allows() {
        let mut s = Sounds::new();
        s.playing = false;
        s.frame_at(0.01, 80, 24);
        for c in PAD_KEYS {
            s.key(KeyCode::Char(c));
        }
        let w = s.frame_at(0.01, 80, 24);
        assert!(w.sounds.len() + w.glass.len() <= 8, "a burst of eight at most");
        // Für Elise over and over, its splashes and its chimes: none that
        // Flow's own allowance (a burst of eight, six a second) would drop.
        let mut s = Sounds::new();
        s.pick(1);
        let (mut left, mut at, mut sent) = (8.0f32, 0.0f32, 0);
        for _ in 0..(30.0 / 0.016) as usize {
            let w = s.frame_at(0.016, 80, 24);
            for _ in w.glass.iter().chain(&w.sounds).filter(|e| e.contains("7731")) {
                left = (left + (s.t - at) * 6.0).min(8.0);
                at = s.t;
                assert!(left >= 1.0, "dropped at {} s", s.t);
                left -= 1.0;
                sent += 1;
            }
        }
        assert!(sent > 100, "{sent} in half a minute");
    }

    #[test]
    fn without_effects_or_flow_only_what_it_may_goes_out() {
        let mut s = Sounds::new();
        s.told(Told { flow: true, effects: false });
        let (glass, heard) = run(&mut s, LEAD + 1.0, 0.01);
        assert!(glass.is_empty() && !heard.is_empty(), "the notes, not the liquid");
        let mut s = Sounds::new();
        s.told(Told { flow: false, effects: false });
        s.frame_at(0.01, 80, 24);
        s.key(KeyCode::Char('a'));
        assert!(s.words().iter().any(|w| w.starts_with("Not in Flow")));
        assert!(s.words()[1].starts_with("Not sent: CSI 5;6;1,~"), "{}", s.words()[1]);
    }

    #[test]
    fn the_screen_fits_its_words_and_says_what_a_program_cant_play() {
        assert_eq!(Layout::new(71, 30), None);
        assert_eq!(Layout::new(80, MIN_ROWS - 1), None);
        let l = Layout::new(80, 24).expect("fits");
        assert_eq!(l.words + WORDS, 24);
        assert!(l.pad(10).0 + PAD_W <= 80 && l.pad(5).0 + PAD_W <= 80 + 1);
        let big = Layout::new(200, 60).expect("fits");
        assert_eq!(big.roll, MAX_ROLL);
        let mut s = Sounds::new();
        s.key(KeyCode::Char('1'));
        s.frame_at(0.01, 80, 24);
        for t in 0..s.tunes.len() {
            s.pick(t);
            for w in s.words() {
                assert!(w.chars().count() <= usize::from(MIN_COLS), "{w}");
            }
        }
        s.play_note(25, 15, true);
        assert!(s.sent.chars().count() + 6 <= usize::from(MIN_COLS), "{}", s.sent);
        assert!(s.words()[4].contains("no program can ask"), "the themes' sounds are Flow's own");
        let first = s.frame_at(0.01, 80, 24).text;
        assert!(!first.contains("\x1b[2J"), "drawn already");
        assert!(s.frame_at(0.01, 60, 20).text.contains("A bigger window"));
    }
}
