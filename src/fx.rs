//! Flow's liquid and its sounds. A program sets off an effect at a cell of
//! its own with OSC 7731 (lightning between two cells, a splash, a ring, a
//! push of the liquid), which also sounds while Flow's sound is on: its
//! kind's sound, none, or one of Flow's by name. It plays one of those names
//! alone with the same escape, and notes with DECPS, as the VT520 did. The
//! same escape washes the glass in a color while the program holds the
//! alternate screen ([`tint`]), as a game's night.
//!
//! A terminal that doesn't know OSC 7731 ignores it, so a program may send
//! effects anywhere, though it's tidier to send them only in Flow
//! ([`crate::in_flow`]). Flow takes them from the tab in front only, about
//! six a second after a burst of eight, and only at cells on the screen
//! shown; on Windows they go down Flow's pipe
//! ([`crate::detect::Terminal::send_effects`]), since ConPTY drops them.
//!
//! Effects that wait for their moment, a beat after what set them off,
//! wait in a [`Queue`]; a [`Budget`] keeps a program under Flow's
//! allowance, so that what it sends isn't dropped unseen.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

/// What an effect does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Lightning from `at` to `to`, which it needs.
    Bolt,
    /// A splash, in its color where the liquid takes color.
    Burst,
    /// A ring spreading.
    Ripple,
    /// The liquid pushed along `dir`, which it needs.
    Stir,
}

impl Kind {
    /// Its name in the escape: `bolt`, `burst`, `ripple` or `stir`.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Bolt => "bolt",
            Kind::Burst => "burst",
            Kind::Ripple => "ripple",
            Kind::Stir => "stir",
        }
    }

    /// The sound Flow gives it unless told otherwise.
    pub fn sound(self) -> Sound {
        match self {
            Kind::Bolt => Sound::Thunder,
            Kind::Burst => Sound::Splash,
            Kind::Ripple => Sound::Drop,
            Kind::Stir => Sound::Whoosh,
        }
    }
}

/// Flow's sounds that a program can name, each made in code as it plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    /// A drop into water: a ripple's.
    Drop,
    /// Water splashing: a burst's.
    Splash,
    /// A crack and rolling thunder: a bolt's.
    Thunder,
    /// Air rushing past: a stir's.
    Whoosh,
    /// Two bell notes rising.
    Chime,
    /// Two soft notes falling.
    Low,
    /// Two bell notes, asking.
    Ding,
    /// A dull knock.
    Thud,
    /// A key's click.
    Click,
    /// A spring's boing.
    Boing,
    /// Flow's own bell.
    Bell,
}

impl Sound {
    /// Every sound, in the order C and Python number them.
    pub const ALL: [Sound; 11] = [Sound::Drop, Sound::Splash, Sound::Thunder, Sound::Whoosh, Sound::Chime, Sound::Low, Sound::Ding, Sound::Thud, Sound::Click, Sound::Boing, Sound::Bell];

    /// Its name in the escape: `drop`, `chime` and so on.
    pub fn name(self) -> &'static str {
        match self {
            Sound::Drop => "drop",
            Sound::Splash => "splash",
            Sound::Thunder => "thunder",
            Sound::Whoosh => "whoosh",
            Sound::Chime => "chime",
            Sound::Low => "low",
            Sound::Ding => "ding",
            Sound::Thud => "thud",
            Sound::Click => "click",
            Sound::Boing => "boing",
            Sound::Bell => "bell",
        }
    }

    /// The sound of that name, in any case; none for a name Flow lacks.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name().eq_ignore_ascii_case(name))
    }

    /// Plays it alone: `ESC ] 7731 ; fx=sound ; sound=<name> [; at=<col>,<row>]
    /// [; strength=<0..1>] BEL`. `at`'s column places it left or right, and
    /// it's louder as it's stronger (0.5 unless given).
    pub fn escape(self, at: Option<(u16, u16)>, strength: Option<f32>) -> String {
        self.escape_with(at, strength, 1.0)
    }

    /// [`Sound::escape`] at `volume` (0 to 1) of what Flow would make it
    /// (`; volume=<0..1>`, written only below full): a program's own volume
    /// for its sounds, as [`Effect::volume`] is for its effects'.
    pub fn escape_with(self, at: Option<(u16, u16)>, strength: Option<f32>, volume: f32) -> String {
        let mut e = format!("\x1b]7731;fx=sound;sound={}", self.name());
        if let Some((col, row)) = at {
            let _ = write!(e, ";at={col},{row}");
        }
        if let Some(s) = strength {
            let _ = write!(e, ";strength={:.2}", s.clamp(0.0, 1.0));
        }
        if volume < 1.0 {
            let _ = write!(e, ";volume={:.2}", volume.max(0.0));
        }
        e.push('\x07');
        e
    }
}

/// What an effect sounds like, while Flow's effects' sounds are on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FxSound {
    /// Its kind's ([`Kind::sound`]).
    #[default]
    Kind,
    /// None (`sound=none`).
    Quiet,
    /// One of Flow's by name.
    Named(Sound),
}

/// An effect at a cell: `ESC ] 7731 ; fx=<kind> ; at=<col>,<row> [; to=<col>,<row>]
/// [; color=<rrggbb>] [; dir=<dx>,<dy>] [; sound=<name>|none] [; size=<cells>]
/// [; strength=<0..1>] BEL`, cells counted from the screen's top-left from
/// 0. Flow ignores keys it doesn't know, and a request of an unknown kind or
/// without what its kind needs.
#[derive(Debug, Clone, PartialEq)]
pub struct Effect {
    /// What it does.
    pub kind: Kind,
    /// Where it's set off: a screen cell's column and row, from 0.
    pub at: (u16, u16),
    /// Where a bolt ends.
    pub to: Option<(u16, u16)>,
    /// 0xRRGGBB.
    pub color: Option<u32>,
    /// Which way a stir pushes, y down as the rows go.
    pub dir: Option<(f32, f32)>,
    /// What it sounds like.
    pub sound: FxSound,
    /// How far it reaches, in columns (0.5 to 40); its kind's own unless
    /// given: 1 for a bolt, 3 a burst, 2 a ripple, 4 a stir.
    pub size: Option<f32>,
    /// How hard, 0 to 1.
    pub strength: f32,
    /// How loud its sound is, 0 to 1, against what its strength makes it:
    /// the program's own volume for its effects, leaving how they look.
    pub volume: f32,
}

impl Effect {
    /// An effect of `kind` at cell `at`, as strong as 0.5, sounding as its
    /// kind does.
    pub fn new(kind: Kind, at: (u16, u16)) -> Self {
        Self { kind, at, to: None, color: None, dir: None, sound: FxSound::Kind, size: None, strength: 0.5, volume: 1.0 }
    }

    /// Lightning from `at` to `to`.
    pub fn bolt(at: (u16, u16), to: (u16, u16)) -> Self {
        Self::new(Kind::Bolt, at).to(to)
    }

    /// A splash at `at`.
    pub fn burst(at: (u16, u16)) -> Self {
        Self::new(Kind::Burst, at)
    }

    /// A ring spreading from `at`.
    pub fn ripple(at: (u16, u16)) -> Self {
        Self::new(Kind::Ripple, at)
    }

    /// The liquid at `at` pushed along `dir`, y down.
    pub fn stir(at: (u16, u16), dir: (f32, f32)) -> Self {
        Self::new(Kind::Stir, at).dir(dir)
    }

    /// Ending at `to`, as a bolt does.
    pub fn to(self, to: (u16, u16)) -> Self {
        Self { to: Some(to), ..self }
    }

    /// In `color`, 0xRRGGBB, where the liquid takes color.
    pub fn color(self, color: u32) -> Self {
        Self { color: Some(color), ..self }
    }

    /// Pushing along `dir`, y down, as a stir does.
    pub fn dir(self, dir: (f32, f32)) -> Self {
        Self { dir: Some(dir), ..self }
    }

    /// Reaching `cells` columns (0.5 to 40).
    pub fn size(self, cells: f32) -> Self {
        Self { size: Some(cells), ..self }
    }

    /// As hard as `strength`, 0 to 1.
    pub fn strength(self, strength: f32) -> Self {
        Self { strength, ..self }
    }

    /// One of Flow's sounds in place of its kind's.
    pub fn sound(self, sound: Sound) -> Self {
        Self { sound: FxSound::Named(sound), ..self }
    }

    /// No sound at all.
    pub fn quiet(self) -> Self {
        Self { sound: FxSound::Quiet, ..self }
    }

    /// Its sound at `volume` (0 to 1) of what Flow would make it.
    pub fn volume(self, volume: f32) -> Self {
        Self { volume, ..self }
    }

    /// The escape, as Flow reads it, ended by BEL.
    pub fn escape(&self) -> String {
        self.escape_as(Spelling::default())
    }

    /// The escape with its numbers written as `spelling` says: Flow reads
    /// them alike either way.
    pub fn escape_as(&self, spelling: Spelling) -> String {
        let mut e = format!("\x1b]7731;fx={};at={},{}", self.kind.name(), self.at.0, self.at.1);
        if let Some((col, row)) = self.to {
            let _ = write!(e, ";to={col},{row}");
        }
        if let Some(c) = self.color {
            let _ = write!(e, ";color={:06x}", c & 0xff_ffff);
        }
        match self.dir {
            Some((x, y)) if spelling.whole_dir => {
                let _ = write!(e, ";dir={},{}", x.round() as i32, y.round() as i32);
            }
            Some((x, y)) => {
                let _ = write!(e, ";dir={x:.2},{y:.2}");
            }
            None => {}
        }
        match self.sound {
            FxSound::Kind => {}
            FxSound::Quiet => e.push_str(";sound=none"),
            FxSound::Named(s) => {
                let _ = write!(e, ";sound={}", s.name());
            }
        }
        if self.volume < 1.0 && spelling.volume_tenths {
            let _ = write!(e, ";volume={:.1}", self.volume.max(0.0));
        } else if self.volume < 1.0 {
            let _ = write!(e, ";volume={:.2}", self.volume.max(0.0));
        }
        if let Some(size) = self.size {
            let _ = write!(e, ";size={:.1}", size.clamp(0.5, 40.0));
        }
        let _ = write!(e, ";strength={:.2}\x07", self.strength.clamp(0.0, 1.0));
        e
    }
}

/// How [`Effect::escape_as`] writes the numbers that Flow reads alike
/// however they're written. The default writes them as [`Effect::escape`]
/// does: `dir` and `volume` to hundredths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Spelling {
    /// `dir` in whole cells (`dir=1,-1`), rounded, for pushes along the
    /// grid.
    pub whole_dir: bool,
    /// `volume` to tenths (`volume=0.3`), for a program whose volume goes
    /// in tenths.
    pub volume_tenths: bool,
}

/// Washes Flow's glass in `color` (0xRRGGBB) while the program holds the
/// alternate screen, the glass shifting toward it and the liquid dimming as
/// far as `strength` (0 to 1) says, as a game's dusk and night do:
/// `ESC ] 7731 ; fx=tint ; color=<rrggbb> ; strength=<0..1> BEL`. Like the
/// effects it's Flow's alone, so send it in Flow ([`crate::in_flow`]), when
/// the wash changes, not each frame; a strength of 0 washes it off
/// ([`UNTINT`]), and so does leaving the alternate screen.
pub fn tint(color: u32, strength: f32) -> String {
    let strength = if strength.is_nan() { 0.0 } else { strength.clamp(0.0, 1.0) };
    if strength <= 0.0 {
        return UNTINT.to_string();
    }
    format!("\x1b]7731;fx=tint;color={:06x};strength={strength:.2}\x07", color & 0xff_ffff)
}

/// Washes Flow's glass clear again: [`tint`] at no strength.
pub const UNTINT: &str = "\x1b]7731;fx=tint;strength=0\x07";

/// Notes as DECPS plays them: `ESC [ <volume> ; <length> ; <note> [; <note> …] , ~`,
/// `volume` 0 (silent) to 7, each note `length` 32nds of a second, and each
/// note from 1 (C5) up a semitone at a time to 25 (C7), 0 a rest (past 25
/// it's a rest too). They follow one another; Flow plays up to ten seconds
/// ahead. Nothing for no notes, which DECPS needs one of.
pub fn notes(volume: u8, length: u8, notes: &[u8]) -> String {
    if notes.is_empty() {
        return String::new();
    }
    let mut e = format!("\x1b[{};{length}", volume.min(7));
    for &n in notes {
        let _ = write!(e, ";{}", if n <= 25 { n } else { 0 });
    }
    e.push_str(",~");
    e
}

/// DECPS's number for a note by name, from `C5` (1) to `C7` (25), with a
/// sharp (`C#5`) or flat (`Db5`); `r` is a rest (0). None past DECPS's two
/// octaves.
pub fn note(name: &str) -> Option<u8> {
    if name == "r" {
        return Some(0);
    }
    let mut chars = name.chars();
    let base: i32 = match chars.next()? {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest = chars.as_str();
    let (shift, octave) = match rest.chars().next()? {
        '#' => (1, &rest[1..]),
        'b' => (-1, &rest[1..]),
        _ => (0, rest),
    };
    let octave: i32 = octave.parse().ok()?;
    let n = (octave - 5) * 12 + base + shift + 1;
    u8::try_from(n).ok().filter(|n| (1..=25).contains(n))
}

/// What Flow lets a tab's program set off, kept under: about six effects
/// and named sounds a second after a burst of eight, of which a budget
/// spends [`Budget::PER_SECOND`] after a burst of [`Budget::BURST`] unless
/// made otherwise, so that nothing a program sends is dropped unseen. Each
/// effect or sound takes one, and they come back as time passes; a program
/// that keeps some back for what matters more (a hit over a spark) asks to
/// keep them ([`Budget::take`]). Time is the program's own clock, in
/// seconds; DECPS notes and the glass's wash don't count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Budget {
    left: f32,
    at: f32,
    per_second: f32,
    burst: f32,
}

impl Default for Budget {
    fn default() -> Self {
        Self::new()
    }
}

impl Budget {
    /// How many come back a second, unless made otherwise: a little under
    /// Flow's six.
    pub const PER_SECOND: f32 = 5.0;
    /// How many may go at once after a rest, unless made otherwise: a
    /// little under Flow's eight.
    pub const BURST: f32 = 7.0;

    /// A full budget of [`Budget::BURST`], coming back at
    /// [`Budget::PER_SECOND`].
    pub fn new() -> Self {
        Self::with(Self::PER_SECOND, Self::BURST)
    }

    /// A full budget of `burst`, coming back at `per_second`.
    pub fn with(per_second: f32, burst: f32) -> Self {
        let burst = if burst.is_finite() { burst.max(0.0) } else { Self::BURST };
        let per_second = if per_second.is_finite() { per_second.max(0.0) } else { Self::PER_SECOND };
        Self { left: burst, at: 0.0, per_second, burst }
    }

    /// How many are left at `now`, in seconds.
    pub fn left(&self, now: f32) -> f32 {
        (self.left + (now - self.at).max(0.0) * self.per_second).min(self.burst)
    }

    /// Spends one at `now`, if `keep` would still be left after it for
    /// what matters more; false, and nothing spent, if not.
    pub fn take(&mut self, now: f32, keep: f32) -> bool {
        self.left = self.left(now);
        self.at = self.at.max(now);
        if self.left - 1.0 >= keep {
            self.left -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Something waiting in a [`Queue`] for its moment.
pub trait Due {
    /// The moment it's due.
    fn due(&self) -> Instant;
}

/// An effect and the moment it's to be set off, for a [`Queue`].
#[derive(Debug, Clone, PartialEq)]
pub struct Timed {
    /// When it's due.
    pub due: Instant,
    /// What's set off then.
    pub effect: Effect,
}

impl Timed {
    /// `effect`, due at `due`.
    pub fn new(due: Instant, effect: Effect) -> Self {
        Self { due, effect }
    }

    /// Its effect's escape.
    pub fn escape(&self) -> String {
        self.effect.escape()
    }
}

impl Due for Timed {
    fn due(&self) -> Instant {
        self.due
    }
}

/// How long past its moment something in a [`Queue`] may still go out,
/// unless the queue says otherwise ([`Queue::late`]).
pub const LATE: Duration = Duration::from_secs(1);

/// Effects waiting for their moment, so that a blast's lightning can run
/// out a beat after the blast: added as the program's events happen, and
/// each frame those whose moment has come taken in the order they were
/// added, to send after the screen that shows what set them off. One that
/// waited past its moment by [`LATE`] or more (behind a menu, say, or a
/// window too small to show where it was) is let go, its moment past.
///
/// What waits is `T`: effects at their cells ([`Timed`]), or whatever a
/// program makes its effects from once it knows where things are drawn, as
/// Flow Rogue's effects wait on tiles of its map and are put in cells as
/// they go out.
#[derive(Debug, Clone)]
pub struct Queue<T = Timed> {
    waiting: Vec<T>,
    late: Duration,
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Queue<T> {
    /// An empty queue, letting go of what's [`LATE`].
    pub fn new() -> Self {
        Self { waiting: Vec::new(), late: LATE }
    }

    /// Letting go of what has waited `late` or more past its moment.
    pub fn late(self, late: Duration) -> Self {
        Self { late, ..self }
    }

    /// Adds `item`, to go out at its moment.
    pub fn add(&mut self, item: T) {
        self.waiting.push(item);
    }

    /// Lets go of everything waiting.
    pub fn clear(&mut self) {
        self.waiting.clear();
    }

    /// What's waiting, in the order it was added.
    pub fn queued(&self) -> &[T] {
        &self.waiting
    }

    /// What's waiting, to change in place: moved with what set it off, say.
    pub fn queued_mut(&mut self) -> &mut [T] {
        &mut self.waiting
    }

    /// How many are waiting.
    pub fn len(&self) -> usize {
        self.waiting.len()
    }

    /// Whether nothing waits.
    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }
}

impl<T: Due> Queue<T> {
    /// When the next one is due, if any is waiting: how long a program
    /// waiting for keys may wait.
    pub fn next(&self) -> Option<Instant> {
        self.waiting.iter().map(Due::due).min()
    }

    /// Takes what's due at `now`, in the order it was added, letting go of
    /// what's waited too long; the rest wait on.
    pub fn take_due(&mut self, now: Instant) -> Vec<T> {
        let (due, later): (Vec<T>, Vec<T>) = std::mem::take(&mut self.waiting).into_iter().partition(|item| item.due() <= now);
        self.waiting = later;
        due.into_iter().filter(|item| now.saturating_duration_since(item.due()) < self.late).collect()
    }
}

impl Queue<Timed> {
    /// The escapes of the effects due at `now` ([`Queue::take_due`]), one
    /// after another.
    pub fn escapes(&mut self, now: Instant) -> String {
        self.take_due(now).iter().map(Timed::escape).collect()
    }

    /// Moves every effect waiting by (`dx`, `dy`) cells, as what set them
    /// off moved when the window changed; none goes past the top left.
    pub fn shift(&mut self, dx: i32, dy: i32) {
        let by = |(x, y): (u16, u16)| ((x as i32 + dx).max(0) as u16, (y as i32 + dy).max(0) as u16);
        for timed in &mut self.waiting {
            timed.effect.at = by(timed.effect.at);
            timed.effect.to = timed.effect.to.map(by);
        }
    }
}

impl<T> Extend<T> for Queue<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, items: I) {
        self.waiting.extend(items);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_wait_for_their_moment_and_go_in_the_order_added() {
        let now = Instant::now();
        let ms = Duration::from_millis;
        let mut queue: Queue = Queue::new();
        assert_eq!((queue.next(), queue.escapes(now)), (None, String::new()));
        queue.add(Timed::new(now + ms(200), Effect::burst((2, 2)).size(6.0)));
        queue.add(Timed::new(now, Effect::ripple((1, 1)).size(3.0)));
        queue.add(Timed::new(now + ms(100), Effect::ripple((5, 5))));
        assert_eq!((queue.next(), queue.len()), (Some(now), 3));
        assert_eq!(queue.escapes(now), "\x1b]7731;fx=ripple;at=1,1;size=3.0;strength=0.50\x07");
        let both = queue.escapes(now + ms(250));
        assert!(both.starts_with("\x1b]7731;fx=burst;at=2,2;") && both.ends_with("fx=ripple;at=5,5;strength=0.50\x07"), "as added, not as due: {both:?}");
        assert!(queue.is_empty());
    }

    #[test]
    fn what_waited_too_long_is_let_go_and_what_waits_moves() {
        let now = Instant::now();
        let mut queue: Queue = Queue::new();
        queue.extend([Timed::new(now, Effect::ripple((1, 1))), Timed::new(now + Duration::from_millis(900), Effect::bolt((3, 4), (10, 1)))]);
        queue.shift(2, -2);
        assert_eq!((queue.queued()[1].effect.at, queue.queued()[1].effect.to), ((5, 2), Some((12, 0))));
        let late = queue.take_due(now + Duration::from_millis(1000));
        assert_eq!(late.len(), 1, "the first waited a second, the second a tenth");
        // A queue of a program's own things, with a lateness of its own.
        struct Tile(Instant);
        impl Due for Tile {
            fn due(&self) -> Instant {
                self.0
            }
        }
        let mut tiles: Queue<Tile> = Queue::new().late(Duration::from_millis(50));
        tiles.add(Tile(now));
        assert!(tiles.take_due(now + Duration::from_millis(60)).is_empty() && tiles.is_empty());
    }

    #[test]
    fn an_effects_volume_goes_out_only_below_full() {
        let e = Effect::new(Kind::Burst, (3, 4));
        assert!(!e.escape().contains("volume"));
        assert!(e.volume(0.4).escape().contains(";volume=0.40;"));
    }

    #[test]
    fn an_effect_is_written_as_flow_reads_it() {
        let bolt = Effect::bolt((3, 4), (10, 4)).color(0xff5a1a).size(2.0).strength(0.8);
        assert_eq!(bolt.escape(), "\x1b]7731;fx=bolt;at=3,4;to=10,4;color=ff5a1a;size=2.0;strength=0.80\x07");
        let stir = Effect::stir((4, 7), (1.0, -1.0)).color(0xe02020).sound(Sound::Thud).size(10.0);
        assert_eq!(stir.escape(), "\x1b]7731;fx=stir;at=4,7;color=e02020;dir=1.00,-1.00;sound=thud;size=10.0;strength=0.50\x07");
        assert_eq!(Effect::ripple((0, 0)).quiet().size(900.0).escape(), "\x1b]7731;fx=ripple;at=0,0;sound=none;size=40.0;strength=0.50\x07", "kept in Flow's range");
    }

    #[test]
    fn numbers_can_be_spelled_whole_or_in_tenths() {
        let stir = Effect::stir((4, 7), (1.0, -1.0)).volume(0.3).size(3.0);
        assert_eq!(stir.escape(), "\x1b]7731;fx=stir;at=4,7;dir=1.00,-1.00;volume=0.30;size=3.0;strength=0.50\x07");
        let short = Spelling { whole_dir: true, volume_tenths: true };
        assert_eq!(stir.escape_as(short), "\x1b]7731;fx=stir;at=4,7;dir=1,-1;volume=0.3;size=3.0;strength=0.50\x07");
        assert_eq!(stir.escape_as(Spelling::default()), stir.escape());
        assert!(Effect::stir((0, 0), (0.6, 0.0)).escape_as(short).contains(";dir=1,0;"), "rounded");
    }

    #[test]
    fn a_sound_alone_goes_by_its_name() {
        assert_eq!(Sound::Chime.escape(Some((30, 4)), Some(0.8)), "\x1b]7731;fx=sound;sound=chime;at=30,4;strength=0.80\x07");
        assert_eq!(Sound::Click.escape(None, None), "\x1b]7731;fx=sound;sound=click\x07");
        assert_eq!(Sound::named("BOING"), Some(Sound::Boing));
        assert_eq!(Sound::named("kazoo"), None);
        assert!(Sound::ALL.iter().all(|s| Sound::named(s.name()) == Some(*s)));
    }

    #[test]
    fn a_sounds_volume_goes_out_only_below_full() {
        assert_eq!(Sound::Ding.escape_with(None, None, 1.0), Sound::Ding.escape(None, None));
        assert_eq!(Sound::Ding.escape_with(Some((2, 3)), Some(0.6), 0.25), "\x1b]7731;fx=sound;sound=ding;at=2,3;strength=0.60;volume=0.25\x07");
        assert!(Sound::Thud.escape_with(None, None, -3.0).ends_with(";volume=0.00\x07"), "never below silent");
    }

    #[test]
    fn a_tint_washes_the_glass_and_none_washes_it_off() {
        assert_eq!(tint(0x0c1838, 0.55), "\x1b]7731;fx=tint;color=0c1838;strength=0.55\x07");
        assert_eq!(tint(0xff00_0000 | 0x6a3a2a, 4.0), "\x1b]7731;fx=tint;color=6a3a2a;strength=1.00\x07", "kept to 24 bits and full strength");
        assert_eq!(tint(0x123456, 0.0), UNTINT);
        assert_eq!(tint(0x123456, f32::NAN), UNTINT);
        assert_eq!(UNTINT, "\x1b]7731;fx=tint;strength=0\x07");
    }

    #[test]
    fn a_budget_spends_a_burst_keeps_some_back_and_fills_again() {
        let mut budget = Budget::new();
        let spent = (0..10).filter(|_| budget.take(0.0, 0.0)).count();
        assert_eq!(spent, 7, "a burst of seven");
        assert!(!budget.take(0.1, 0.0), "half of one back in a tenth of a second");
        assert!(budget.take(0.2, 0.0), "one back in a fifth");
        let mut kept = Budget::new();
        assert_eq!((0..10).filter(|_| kept.take(0.0, 2.0)).count(), 5, "two kept back for what matters more");
        assert!(kept.take(0.0, 0.0) && kept.take(0.0, 0.0) && !kept.take(0.0, 0.0));
        assert_eq!(kept.left(10.0), Budget::BURST, "full again, no fuller");
        let mut own = Budget::with(1.0, 2.0);
        assert!(own.take(5.0, 0.0) && own.take(5.0, 0.0) && !own.take(5.5, 0.0) && own.take(6.0, 0.0));
        assert!(!own.take(3.0, 0.0), "time going back gives nothing back");
    }

    #[test]
    fn notes_go_as_decps_has_them() {
        let tune: Vec<u8> = ["C5", "E5", "G5", "r", "C6", "C7"].iter().map(|n| note(n).unwrap()).collect();
        assert_eq!(tune, [1, 5, 8, 0, 13, 25]);
        assert_eq!(notes(9, 8, &tune), "\x1b[7;8;1;5;8;0;13;25,~", "the volume tops out at 7");
        assert_eq!(notes(3, 16, &[30]), "\x1b[3;16;0,~", "past 25 is a rest");
        assert_eq!(notes(3, 16, &[]), "");
        assert_eq!((note("Db5"), note("C#5"), note("B4"), note("C#7"), note("H5")), (Some(2), Some(2), None, None, None));
    }
}
