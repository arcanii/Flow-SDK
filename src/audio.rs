//! Sound of the program's own, behind the `audio` feature: tones and short
//! tunes made in code and played on the machine, and songs for a small band
//! of instruments (a plucked lute, a recorder, a drone and a tabor), looped
//! and faded one into the next, apart from Flow's sounds (which Flow plays,
//! and a program asks for with [`crate::fx`]). It plays wherever the
//! program runs, in any terminal or none: on macOS (CoreAudio) and Windows
//! (WASAPI) through cpal, and on Linux through ALSA's library, loaded as
//! the program runs (`alsa.rs`), where it's there.
//!
//! [`Audio`] hands what's wanted to a thread of its own, which opens the
//! system's output when there's something to play and closes it after a few
//! seconds of silence. [`Mixer`] is the sound itself, the voices summed
//! with a soft limit, rendered sample by sample in the output's callback
//! without waiting on anything; a song's voices are made before it's handed
//! over. Tests run it without an output.
//!
//! A [`Clip`] is sound a program has made itself, sample by sample (speech
//! it synthesizes, say), played once at its own rate alongside the rest.

use std::f32::consts::TAU;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Whether anything plays on this system: on Linux, as long as ALSA's
/// library is there, which desktops have.
pub const AVAILABLE: bool = cfg!(any(unix, windows));

/// Voices sounding at once, at most; a new one past them takes the place
/// of the oldest.
const MAX_VOICES: usize = 32;
/// Tunes playing at once, at most.
const MAX_TUNES: usize = 8;
/// Clips playing at once, at most; a new one past them takes the place of
/// the oldest.
const MAX_CLIPS: usize = 8;
/// Silence after which the output is closed.
const IDLE: Duration = Duration::from_secs(3);
/// How long after the output wouldn't open it's tried again, rather than for
/// everything asked.
const RETRY: Duration = Duration::from_secs(10);

/// A tone's shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wave {
    /// A pure tone.
    Sine,
    /// Softer than a square, hollow.
    Triangle,
    /// Buzzing, as old machines beeped.
    Square,
    /// Bright and brassy.
    Saw,
    /// White noise, for drums and wind.
    Noise,
}

/// One note: its pitch and how long it's held, how it sounds, and how it
/// comes in and dies away.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tone {
    /// Its pitch in hertz ([`pitch`] names one).
    pub hz: f32,
    /// How long it's held, in seconds, before it dies away.
    pub secs: f32,
    /// Its shape.
    pub wave: Wave,
    /// 0 to 1.
    pub volume: f32,
    /// Left (-1) to right (1).
    pub pan: f32,
    /// Seconds to come in.
    pub attack: f32,
    /// Seconds to die away after it's held.
    pub release: f32,
}

impl Tone {
    /// `hz` held `secs` seconds: a sine at half volume in the middle, in
    /// over 5 ms and away over 80.
    pub fn new(hz: f32, secs: f32) -> Self {
        Self { hz, secs, wave: Wave::Sine, volume: 0.5, pan: 0.0, attack: 0.005, release: 0.08 }
    }

    /// In another shape.
    pub fn wave(self, wave: Wave) -> Self {
        Self { wave, ..self }
    }

    /// As loud as `volume`, 0 to 1.
    pub fn volume(self, volume: f32) -> Self {
        Self { volume, ..self }
    }

    /// Placed from left (-1) to right (1).
    pub fn pan(self, pan: f32) -> Self {
        Self { pan, ..self }
    }

    /// Coming in over `attack` seconds and dying away over `release`.
    pub fn envelope(self, attack: f32, release: f32) -> Self {
        Self { attack, release, ..self }
    }

    /// How long it sounds, release and all.
    pub fn length(&self) -> f32 {
        self.secs + self.release
    }
}

/// A note's pitch in hertz from its name: a letter, a sharp (`#`) or flat
/// (`b`), and the octave, `C4` being middle C and `A4` 440.
pub fn pitch(name: &str) -> Option<f32> {
    let mut chars = name.chars();
    let base = match chars.next()? {
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
    let midi = 12 * (octave + 1) + base + shift;
    Some(440.0 * 2f32.powf((midi - 69) as f32 / 12.0))
}

/// A step of a tune: a pitch, or none for a rest, and how many beats.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    /// Its pitch in hertz; none for a rest.
    pub hz: Option<f32>,
    /// How many beats it lasts.
    pub beats: f32,
}

/// A line of notes, played one after another on one sound.
#[derive(Debug, Clone, PartialEq)]
pub struct Tune {
    /// Its notes and rests, in order.
    pub steps: Vec<Step>,
    /// Beats a minute.
    pub tempo: f32,
    /// How each note sounds; its pitch and length are the steps'.
    pub sound: Tone,
    /// Plays again from the start until stopped.
    pub looped: bool,
}

impl Tune {
    /// A tune from notes written as `name/beats`, `r/beats` a rest:
    /// `"C4/1 E4/1 G4/2 r/1"`.
    pub fn parse(notes: &str, tempo: f32) -> Result<Self, String> {
        let steps = notes
            .split_whitespace()
            .map(|token| {
                let (name, beats) = token.split_once('/').ok_or(format!("{token}: a note needs its length"))?;
                let beats: f32 = beats.parse().map_err(|_| format!("{token}: its length isn't a number"))?;
                let hz = if name == "r" { None } else { Some(pitch(name).ok_or(format!("{token}: no such note"))?) };
                Ok(Step { hz, beats })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self { steps, tempo, sound: Tone::new(0.0, 0.0), looped: false })
    }

    /// Each note sounding as `sound` does, but for its pitch and length.
    pub fn sound(self, sound: Tone) -> Self {
        Self { sound, ..self }
    }

    /// Played again from the start until stopped.
    pub fn looped(self) -> Self {
        Self { looped: true, ..self }
    }

    /// Seconds a beat lasts.
    fn beat(&self) -> f32 {
        60.0 / self.tempo.max(1.0)
    }

    /// How long once through takes.
    pub fn length(&self) -> f32 {
        self.steps.iter().map(|s| s.beats).sum::<f32>() * self.beat()
    }
}

/// An instrument a [`Song`]'s line is played on, each made in code as it
/// plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instrument {
    /// A plucked string (Karplus and Strong's), ringing on after it's let
    /// go: a lute, or a harp.
    Lute,
    /// A breathy pipe, with a vibrato once a note has settled.
    Recorder,
    /// A note and the fifth above, reedy and soft, swelling slowly: held
    /// under a tune.
    Drone,
    /// A skin struck, its pitch falling away, and a slap: a drum, whose
    /// notes are how hard it's struck.
    Tabor,
}

impl Instrument {
    /// Every instrument, in the order C and Python number them.
    pub const ALL: [Instrument; 4] = [Instrument::Lute, Instrument::Recorder, Instrument::Drone, Instrument::Tabor];

    /// How loud it plays in a song, and where it stands, left (-1) to right
    /// (1), as a small band would.
    pub fn mix(self) -> (f32, f32) {
        match self {
            Instrument::Lute => (0.55, -0.25),
            Instrument::Recorder => (0.32, 0.25),
            Instrument::Drone => (0.1, 0.0),
            Instrument::Tabor => (0.45, 0.1),
        }
    }

    /// Voices a line on it keeps, so a note rings on under the next.
    pub fn voices(self) -> usize {
        match self {
            Instrument::Lute => 6,
            Instrument::Recorder => 2,
            Instrument::Drone => 2,
            Instrument::Tabor => 3,
        }
    }
}

/// One instrument's part in a [`Song`]: its notes, one after another, and
/// how loud it is against the instrument's own level.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// What plays it.
    pub instrument: Instrument,
    /// How loud, against the instrument's own level (1).
    pub gain: f32,
    /// The notes. On the tabor a step's `hz` is how hard it's struck, 0 to 1.
    pub steps: Vec<Step>,
}

impl Line {
    /// A line from notes written as `name/beats` (`D4/3`, `Bb3/2`), `r/beats`
    /// a rest; on the tabor `X/beats` is a full beat and `x/beats` a light
    /// one.
    pub fn parse(instrument: Instrument, gain: f32, notes: &str) -> Result<Self, String> {
        let steps = notes
            .split_whitespace()
            .map(|token| {
                let (name, beats) = token.split_once('/').ok_or(format!("{token}: a note needs its length"))?;
                let beats: f32 = beats.parse().map_err(|_| format!("{token}: its length isn't a number"))?;
                let hz = match (instrument, name) {
                    (_, "r") => None,
                    (Instrument::Tabor, "X") => Some(1.0),
                    (Instrument::Tabor, "x") => Some(0.55),
                    (_, name) => Some(pitch(name).ok_or(format!("{token}: no such note"))?),
                };
                Ok(Step { hz, beats })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self { instrument, gain, steps })
    }

    /// Its beats, once through.
    pub fn beats(&self) -> f32 {
        self.steps.iter().map(|s| s.beats).sum()
    }
}

/// A piece for several instruments at once, played looped: its lines side
/// by side, each as many beats long as the others, at `tempo` beats a
/// minute. [`Mixer::play_song`] fades one song into the next, as a game's
/// music goes from place to place.
#[derive(Debug, Clone, PartialEq)]
pub struct Song {
    /// Beats a minute.
    pub tempo: f32,
    /// The instruments' parts, played together.
    pub lines: Vec<Line>,
}

impl Song {
    /// `lines` played together at `tempo` beats a minute.
    pub fn new(tempo: f32, lines: Vec<Line>) -> Self {
        Self { tempo, lines }
    }

    /// How long once through takes: its longest line's beats.
    pub fn length(&self) -> f32 {
        self.lines.iter().map(Line::beats).fold(0.0, f32::max) * 60.0 / self.tempo.max(1.0)
    }
}

/// Sound a program has made itself, sample by sample, or decoded (speech
/// it synthesizes, music it carries), played at its own rate
/// ([`Mixer::play_clip`]): once, or round and round until it's faded out
/// ([`Clip::looped`], [`Mixer::fade_clips`]); in one channel, panned, or
/// two ([`Clip::stereo`]). Its samples are shared, so handing it over
/// copies nothing; keep one to play again, and the output's callback frees
/// nothing when it's done with it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Clip {
    /// Its samples, -1 to 1: two channels' interleaved, left first, for a
    /// clip in stereo.
    pub samples: Arc<[f32]>,
    /// Frames a second (a sample a channel each): the mix plays it at its
    /// own rate, whatever the output's.
    pub rate: f32,
    /// 1, or 2 in stereo.
    pub channels: u8,
    /// 0 to 1.
    pub volume: f32,
    /// -1 (left) to 1 (right): where a clip of one channel sits, and how a
    /// stereo one leans.
    pub pan: f32,
    /// Played round and round, until it's faded out.
    pub looped: bool,
    /// Seconds it comes up over, from silence.
    pub fade_in: f32,
    /// What [`Mixer::fade_clips`] knows it by (0 for none): music, say,
    /// faded apart from speech.
    pub tag: u32,
}

impl Clip {
    /// `samples` of one channel at `rate` a second, at full volume, in the
    /// middle, played once.
    pub fn new(samples: impl Into<Arc<[f32]>>, rate: f32) -> Self {
        Self { samples: samples.into(), rate: rate.max(1.0), channels: 1, volume: 1.0, pan: 0.0, looped: false, fade_in: 0.0, tag: 0 }
    }

    /// `samples` of two channels, interleaved, left first, at `rate`
    /// frames a second.
    pub fn stereo(samples: impl Into<Arc<[f32]>>, rate: f32) -> Self {
        Self { channels: 2, ..Self::new(samples, rate) }
    }

    /// At `volume`, 0 to 1.
    pub fn volume(self, volume: f32) -> Self {
        Self { volume: volume.clamp(0.0, 1.0), ..self }
    }

    /// Panned, -1 (left) to 1 (right).
    pub fn pan(self, pan: f32) -> Self {
        Self { pan: pan.clamp(-1.0, 1.0), ..self }
    }

    /// Played round and round, seamlessly, until it's faded out.
    pub fn looped(self) -> Self {
        Self { looped: true, ..self }
    }

    /// Coming up over `secs` seconds, from silence.
    pub fn fade_in(self, secs: f32) -> Self {
        Self { fade_in: secs.max(0.0), ..self }
    }

    /// Known by `tag` to [`Mixer::fade_clips`].
    pub fn tag(self, tag: u32) -> Self {
        Self { tag, ..self }
    }

    /// Its frames: samples a channel.
    fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    /// How long it plays once round, in seconds.
    pub fn length(&self) -> f32 {
        self.frames() as f32 / self.rate
    }
}

/// A clip playing: how far into its frames it is, how loud it's come up,
/// and how fast it's fading out, if it is.
#[derive(Debug, Clone)]
struct Clipping {
    clip: Clip,
    at: f64,
    level: f32,
    fading: Option<f32>,
}

impl Clipping {
    fn new(clip: Clip) -> Self {
        let level = if clip.fade_in > 0.0 { 0.0 } else { 1.0 };
        Self { clip, at: 0.0, level, fading: None }
    }

    /// Its next left and right, `step` of its own frames on (`dt` seconds),
    /// each between the two frames it falls between, round to the start
    /// for a loop.
    fn next(&mut self, step: f64, dt: f32) -> [f32; 2] {
        let frames = self.clip.frames();
        let channels = self.clip.channels.max(1) as usize;
        let i = self.at as usize;
        let k = (self.at - i as f64) as f32;
        let j = if self.clip.looped && i + 1 >= frames { 0 } else { i + 1 };
        let s = &self.clip.samples;
        let sample = |frame: usize, c: usize| s.get(frame * channels + c.min(channels - 1)).copied().unwrap_or(0.0);
        let [l, r] = [0, 1].map(|c| sample(i, c) + (sample(j, c) - sample(i, c)) * k);
        self.at += step;
        if self.clip.looped && frames > 0 && self.at >= frames as f64 {
            self.at -= frames as f64;
        }
        self.level = match self.fading {
            Some(rate) => (self.level - rate * dt).max(0.0),
            None if self.clip.fade_in > 0.0 => (self.level + dt / self.clip.fade_in).min(1.0),
            None => 1.0,
        };
        let pan = self.clip.pan;
        let gain = self.clip.volume * self.level;
        if channels == 1 {
            // An equal-power pan.
            let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
            [l * gain * angle.cos(), l * gain * angle.sin()]
        } else {
            [l * gain * (1.0 - pan).min(1.0), r * gain * (1.0 + pan).min(1.0)]
        }
    }

    fn done(&self) -> bool {
        (!self.clip.looped && self.at >= self.clip.frames() as f64) || (self.fading.is_some() && self.level <= 0.0)
    }
}

/// One tone sounding.
#[derive(Debug, Clone)]
struct Voice {
    tone: Tone,
    t: f32,
    phase: f32,
    seed: u32,
}

impl Voice {
    fn new(tone: Tone) -> Self {
        Self { tone, t: 0.0, phase: 0.0, seed: 0x1234_5679 }
    }

    fn done(&self) -> bool {
        self.t >= self.tone.length()
    }

    /// Its next sample, `dt` seconds on.
    fn next(&mut self, dt: f32) -> f32 {
        let Tone { hz, secs, wave, volume, attack, release, .. } = self.tone;
        let t = self.t;
        self.t += dt;
        self.phase = (self.phase + hz * dt).fract();
        let p = self.phase;
        let s = match wave {
            Wave::Sine => (TAU * p).sin(),
            Wave::Triangle => 1.0 - 4.0 * (p - 0.5).abs(),
            Wave::Square => {
                if p < 0.5 {
                    0.6
                } else {
                    -0.6
                }
            }
            Wave::Saw => (2.0 * p - 1.0) * 0.6,
            Wave::Noise => {
                let mut x = self.seed;
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                self.seed = x;
                x as f32 / u32::MAX as f32 * 2.0 - 1.0
            }
        };
        let rise = if attack > 0.0 { (t / attack).min(1.0) } else { 1.0 };
        let fall = if t <= secs { 1.0 } else { (1.0 - (t - secs) / release.max(1e-4)).max(0.0) };
        s * rise * fall * volume
    }
}

/// A tune playing: where it is, and samples until its next step.
#[derive(Debug, Clone)]
struct Playing {
    tune: Tune,
    next: usize,
    wait: f64,
}

/// The voices, tunes and song playing, summed with a soft limit, at `rate`
/// samples a second.
#[derive(Debug)]
pub struct Mixer {
    rate: f32,
    voices: Vec<Voice>,
    tunes: Vec<Playing>,
    clips: Vec<Clipping>,
    volume: f32,
    /// The song playing, and the one fading out under it.
    song: Option<Box<Band>>,
    leaving: Option<Box<Band>>,
}

impl Mixer {
    /// A silent mix at `rate` samples a second.
    pub fn new(rate: f32) -> Self {
        Self { rate, voices: Vec::with_capacity(MAX_VOICES), tunes: Vec::with_capacity(MAX_TUNES), clips: Vec::with_capacity(MAX_CLIPS), volume: 1.0, song: None, leaving: None }
    }

    /// Plays `song`, looped, fading it in over a second as the song before
    /// it fades out over a little more. Its voices are made here, so an
    /// output's callback should be handed it made ([`Audio::play_song`]
    /// does that).
    pub fn play_song(&mut self, song: &Song) {
        self.take_band(Box::new(Band::new(song, self.rate)));
    }

    fn take_band(&mut self, band: Box<Band>) {
        self.stop_song();
        self.song = Some(band);
    }

    /// The song playing fades out.
    pub fn stop_song(&mut self) {
        if let Some(mut old) = self.song.take() {
            old.fading_out = true;
            // One still fading goes at once: dropped here, its voices are
            // few and small.
            self.leaving = Some(old);
        }
    }

    /// Sounds `tone` alongside whatever else is playing; the oldest voice
    /// gives way when there are too many.
    pub fn play(&mut self, tone: Tone) {
        if self.voices.len() >= MAX_VOICES {
            self.voices.remove(0);
        }
        self.voices.push(Voice::new(tone));
    }

    /// Plays `tune`'s notes one after another, alongside whatever else is
    /// playing.
    pub fn play_tune(&mut self, tune: Tune) {
        if tune.steps.is_empty() {
            return;
        }
        if self.tunes.len() >= MAX_TUNES {
            self.tunes.remove(0);
        }
        self.tunes.push(Playing { tune, next: 0, wait: 0.0 });
    }

    /// Plays `clip` (once, or looped), alongside whatever else is
    /// playing; the oldest clip gives way when there are too many.
    pub fn play_clip(&mut self, clip: Clip) {
        if clip.frames() == 0 {
            return;
        }
        if self.clips.len() >= MAX_CLIPS {
            self.clips.remove(0);
        }
        self.clips.push(Clipping::new(clip));
    }

    /// The clips tagged `tag` fade out over `secs` seconds and stop (at
    /// once for 0), loops too; the rest play on.
    pub fn fade_clips(&mut self, tag: u32, secs: f32) {
        for c in self.clips.iter_mut().filter(|c| c.clip.tag == tag) {
            c.fading = Some(if secs > 0.0 { c.level / secs } else { f32::INFINITY });
            if secs <= 0.0 {
                c.level = 0.0;
            }
        }
        self.clips.retain(|c| !c.done());
    }

    /// Everything falls silent at once, songs and clips too.
    pub fn stop(&mut self) {
        self.voices.clear();
        self.tunes.clear();
        self.clips.clear();
        self.song = None;
        self.leaving = None;
    }

    /// The whole mix's volume, 0 to 1.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Something is still sounding or still to come: a song, until it's
    /// faded out.
    pub fn busy(&self) -> bool {
        !self.voices.is_empty() || !self.tunes.is_empty() || !self.clips.is_empty() || self.song.is_some() || self.leaving.is_some()
    }

    /// Fills `out`, `channels` samples a frame (left and right first; any
    /// more are left silent, and one alone gets both).
    pub fn fill(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let (rate, dt) = (self.rate, 1.0 / self.rate);
        for frame in out.chunks_mut(channels) {
            self.step_tunes();
            let mut lr = [0.0f32; 2];
            for v in &mut self.voices {
                let s = v.next(dt);
                // An equal-power pan.
                let angle = (v.tone.pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
                lr[0] += s * angle.cos();
                lr[1] += s * angle.sin();
            }
            self.voices.retain(|v| !v.done());
            for c in &mut self.clips {
                let [l, r] = c.next(c.clip.rate as f64 / rate as f64, dt);
                lr[0] += l;
                lr[1] += r;
            }
            self.clips.retain(|c| !c.done());
            for band in [&mut self.song, &mut self.leaving].into_iter().flatten() {
                let [l, r] = band.next(dt, rate);
                lr[0] += l;
                lr[1] += r;
            }
            let (l, r) = ((lr[0] * self.volume).tanh(), (lr[1] * self.volume).tanh());
            match frame {
                [only] => *only = (l + r) * 0.5,
                [a, b, rest @ ..] => {
                    (*a, *b) = (l, r);
                    rest.fill(0.0);
                }
                [] => {}
            }
        }
        if self.leaving.as_ref().is_some_and(|b| b.done()) {
            self.leaving = None;
        }
    }

    /// Starts the notes whose time has come.
    fn step_tunes(&mut self) {
        let rate = self.rate as f64;
        let mut started: [Option<Tone>; MAX_TUNES] = [None; MAX_TUNES];
        for (i, p) in self.tunes.iter_mut().enumerate() {
            if p.wait > 0.0 {
                p.wait -= 1.0;
                continue;
            }
            let step = p.tune.steps[p.next];
            let secs = step.beats * p.tune.beat();
            p.wait += secs as f64 * rate - 1.0;
            p.next += 1;
            if let Some(hz) = step.hz {
                started[i] = Some(Tone { hz, secs: (secs - p.tune.sound.release * 0.5).max(0.01), ..p.tune.sound });
            }
        }
        for tone in started.into_iter().flatten() {
            self.play(tone);
        }
        // A tune done stops, or plays again from the start.
        self.tunes.retain_mut(|p| {
            if p.next < p.tune.steps.len() {
                return true;
            }
            p.next = 0;
            p.tune.looped
        });
    }
}

/// The longest string the lute plucks, in samples: long enough for D2 at
/// 96 kHz.
const STRING: usize = 4096;

/// One note sounding on an instrument.
struct Sounding {
    instrument: Instrument,
    on: bool,
    hz: f32,
    /// How hard, 0 to 1 (the tabor's beat).
    hard: f32,
    /// Seconds since it began, and how long it's held.
    t: f32,
    hold: f32,
    phase: f32,
    phase2: f32,
    seed: u32,
    low: [f32; 3],
    /// The plucked string: its samples, how many of them it rings over,
    /// and where it's read.
    string: Box<[f32; STRING]>,
    period: usize,
    at: usize,
}

impl Sounding {
    fn new(instrument: Instrument) -> Self {
        Self { instrument, on: false, hz: 0.0, hard: 1.0, t: 0.0, hold: 0.0, phase: 0.0, phase2: 0.0, seed: 0x1234_5679, low: [0.0; 3], string: Box::new([0.0; STRING]), period: 2, at: 0 }
    }

    fn start(&mut self, hz: f32, hold: f32, rate: f32) {
        self.on = true;
        self.t = 0.0;
        self.hold = hold;
        self.phase = 0.0;
        self.phase2 = 0.0;
        self.low = [0.0; 3];
        if self.instrument == Instrument::Tabor {
            (self.hz, self.hard) = (0.0, hz);
            return;
        }
        self.hz = hz;
        if self.instrument == Instrument::Lute {
            // The string starts as soft noise, a period long: half a sample
            // short, as averaging its neighbours delays it by half.
            self.period = ((rate / hz - 0.5).round() as usize).clamp(2, STRING);
            let mut last = 0.0;
            for i in 0..self.period {
                let n = self.noise();
                last = (last + n) * 0.5;
                self.string[i] = last;
            }
            self.at = 0;
        }
    }

    fn noise(&mut self) -> f32 {
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        x as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    fn low_pass(&mut self, i: usize, x: f32, hz: f32, dt: f32) -> f32 {
        let k = 1.0 - (-TAU * hz * dt).exp();
        self.low[i] += k * (x - self.low[i]);
        self.low[i]
    }

    /// Its next sample, `dt` seconds on; it goes quiet when done.
    fn next(&mut self, dt: f32) -> f32 {
        let t = self.t;
        self.t += dt;
        let s = match self.instrument {
            Instrument::Lute => {
                let (a, b) = (self.string[self.at], self.string[(self.at + 1) % self.period]);
                self.string[self.at] = (a + b) * 0.5 * 0.9965;
                self.at = (self.at + 1) % self.period;
                let release = ((self.hold + 0.6 - t) / 0.2).clamp(0.0, 1.0);
                if release <= 0.0 {
                    self.on = false;
                }
                self.low_pass(0, a, 3200.0, dt) * release * 1.6
            }
            Instrument::Recorder => {
                // A breathy pipe: a sine and a little of its octaves, a
                // vibrato once the note has settled, and some breath.
                let vibrato = 1.0 + 0.004 * (TAU * 5.5 * t).sin() * ((t - 0.25) / 0.3).clamp(0.0, 1.0);
                self.phase = (self.phase + self.hz * vibrato * dt).fract();
                let x = TAU * self.phase;
                let tone = x.sin() + 0.12 * (2.0 * x).sin() + 0.05 * (3.0 * x).sin();
                let n = self.noise();
                let breath = (n - self.low_pass(0, n, 1800.0, dt)) * 0.05;
                let env = (t / 0.05).min(1.0) * ((self.hold - t) / 0.08 + 1.0).clamp(0.0, 1.0);
                if t > self.hold + 0.08 {
                    self.on = false;
                }
                (tone + breath) * env
            }
            Instrument::Drone => {
                // The note and its fifth, reedy and soft, swelling slowly.
                self.phase = (self.phase + self.hz * dt).fract();
                self.phase2 = (self.phase2 + self.hz * 1.5 * dt).fract();
                let saw = (self.phase * 2.0 - 1.0) + 0.7 * (self.phase2 * 2.0 - 1.0);
                let warm = self.low_pass(0, saw, 520.0, dt);
                let env = (t / 0.8).min(1.0) * ((self.hold + 0.6 - t) / 0.6).clamp(0.0, 1.0);
                if t > self.hold + 0.6 {
                    self.on = false;
                }
                warm * env * (1.0 + 0.1 * (TAU * 0.2 * t).sin())
            }
            Instrument::Tabor => {
                // A skin struck: its pitch falling away, and a slap.
                let hz = 60.0 + 90.0 * (-t / 0.05).exp();
                self.phase = (self.phase + hz * dt).fract();
                let body = (TAU * self.phase).sin() * (-t / 0.15).exp();
                let n = self.noise();
                let slap = self.low_pass(0, n, 1200.0, dt) * (-t / 0.03).exp() * 2.0;
                if t > 0.45 {
                    self.on = false;
                }
                (body + slap) * self.hard
            }
        };
        if self.on {
            s
        } else {
            0.0
        }
    }
}

/// A line of a song playing: where it is, and its voices.
struct LinePlaying {
    instrument: Instrument,
    gain: f32,
    steps: Vec<Step>,
    next: usize,
    /// Samples until its next note.
    wait: f64,
    voices: Vec<Sounding>,
    turn: usize,
}

/// A song playing, made whole (its voices and their strings) before it's
/// handed to the output's callback, and how loud it is as it fades in or
/// out.
struct Band {
    lines: Vec<LinePlaying>,
    /// Samples a beat lasts.
    beat: f64,
    level: f32,
    fading_out: bool,
}

impl std::fmt::Debug for Band {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Band").field("lines", &self.lines.len()).field("level", &self.level).field("fading_out", &self.fading_out).finish()
    }
}

impl Band {
    fn new(song: &Song, rate: f32) -> Self {
        let lines = song
            .lines
            .iter()
            .map(|line| LinePlaying {
                instrument: line.instrument,
                gain: line.gain,
                steps: line.steps.clone(),
                next: 0,
                wait: 0.0,
                voices: (0..line.instrument.voices()).map(|_| Sounding::new(line.instrument)).collect(),
                turn: 0,
            })
            .collect();
        Self { lines, beat: rate as f64 * 60.0 / song.tempo as f64, level: 0.0, fading_out: false }
    }

    /// Its next sample, left and right: in over a second, out over a
    /// little more.
    fn next(&mut self, dt: f32, rate: f32) -> [f32; 2] {
        let step = dt / if self.fading_out { 1.2 } else { 1.0 };
        self.level = if self.fading_out { (self.level - step).max(0.0) } else { (self.level + step).min(1.0) };
        let mut out = [0.0; 2];
        for line in &mut self.lines {
            if line.steps.is_empty() {
                continue;
            }
            if line.wait <= 0.0 {
                let step = line.steps[line.next];
                line.next = (line.next + 1) % line.steps.len();
                let length = step.beats as f64 * self.beat;
                line.wait += length;
                if let Some(hz) = step.hz {
                    let v = line.turn;
                    line.turn = (line.turn + 1) % line.voices.len();
                    line.voices[v].start(hz, (length / rate as f64) as f32, rate);
                }
            }
            line.wait -= 1.0;
            let (level, pan) = line.instrument.mix();
            let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
            let mut s = 0.0;
            for v in line.voices.iter_mut().filter(|v| v.on) {
                s += v.next(dt);
            }
            s *= level * line.gain * self.level;
            out[0] += s * angle.cos();
            out[1] += s * angle.sin();
        }
        out
    }

    fn done(&self) -> bool {
        self.fading_out && self.level <= 0.0
    }
}

/// What the output thread is told, and what it tells the callback.
#[cfg_attr(not(any(unix, windows)), allow(dead_code))]
enum Cmd {
    Tone(Tone),
    Tune(Box<Tune>),
    Clip(Box<Clip>),
    FadeClips(u32, f32),
    Stop,
    Volume(f32),
    /// A song to play, made into a [`Band`] at the output's rate on the
    /// output's thread, so the callback allocates nothing for it.
    Song(Box<Song>),
    Band(Box<Band>),
    StopSong,
}

/// The program's sound: what it asks for goes to a thread of its own,
/// started with the first sound, which opens the system's output then.
pub struct Audio {
    tx: Option<Sender<Cmd>>,
}

impl Audio {
    /// The program's sound, silent: nothing starts until something plays.
    pub fn new() -> Self {
        Self { tx: None }
    }

    /// Sounds `tone` ([`Mixer::play`]).
    pub fn play(&mut self, tone: Tone) {
        self.send(Cmd::Tone(tone));
    }

    /// Plays `tune` ([`Mixer::play_tune`]).
    pub fn play_tune(&mut self, tune: Tune) {
        self.send(Cmd::Tune(Box::new(tune)));
    }

    /// Plays `clip` ([`Mixer::play_clip`]).
    pub fn play_clip(&mut self, clip: Clip) {
        self.send(Cmd::Clip(Box::new(clip)));
    }

    /// The clips tagged `tag` fade out over `secs` seconds and stop
    /// ([`Mixer::fade_clips`]).
    pub fn fade_clips(&mut self, tag: u32, secs: f32) {
        if self.tx.is_some() {
            self.send(Cmd::FadeClips(tag, secs));
        }
    }

    /// Everything falls silent.
    pub fn stop(&mut self) {
        if self.tx.is_some() {
            self.send(Cmd::Stop);
        }
    }

    /// Plays `song` looped, fading in as the song before it fades out
    /// ([`Mixer::play_song`]).
    pub fn play_song(&mut self, song: Song) {
        self.send(Cmd::Song(Box::new(song)));
    }

    /// The song playing fades out.
    pub fn stop_song(&mut self) {
        if self.tx.is_some() {
            self.send(Cmd::StopSong);
        }
    }

    /// The whole mix's volume, 0 to 1 (1 unless set).
    pub fn set_volume(&mut self, volume: f32) {
        self.send(Cmd::Volume(volume));
    }

    fn send(&mut self, cmd: Cmd) {
        if !AVAILABLE {
            return;
        }
        let tx = self.tx.get_or_insert_with(|| {
            let (tx, rx) = mpsc::channel();
            let _ = std::thread::Builder::new().name("flow-sdk-audio".into()).spawn(move || run(rx));
            tx
        });
        let _ = tx.send(cmd);
    }
}

impl Default for Audio {
    fn default() -> Self {
        Self::new()
    }
}

/// The output's thread: opens the output when there's something to play,
/// passes on what's asked, and closes it after [`IDLE`] of silence. It ends
/// when the [`Audio`] is dropped.
fn run(rx: Receiver<Cmd>) {
    let mut output: Option<output::Output> = None;
    let mut failed_at: Option<Instant> = None;
    let mut volume = 1.0;
    let mut quiet_for = Duration::ZERO;
    let tick = Duration::from_millis(250);
    loop {
        match rx.recv_timeout(tick) {
            Ok(Cmd::Volume(v)) => {
                volume = v;
                if let Some(out) = &output {
                    out.send(Cmd::Volume(v));
                }
            }
            Ok(cmd @ (Cmd::Stop | Cmd::StopSong | Cmd::FadeClips(..))) => {
                if let Some(out) = &output {
                    out.send(cmd);
                }
            }
            Ok(Cmd::Song(song)) => {
                quiet_for = Duration::ZERO;
                ready(&mut output, &mut failed_at, volume);
                if let Some(out) = &output {
                    out.send(Cmd::Band(Box::new(Band::new(&song, out.rate))));
                }
            }
            Ok(cmd) => {
                quiet_for = Duration::ZERO;
                ready(&mut output, &mut failed_at, volume);
                if let Some(out) = &output {
                    out.send(cmd);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if output.as_ref().is_some_and(|o| !o.busy()) {
                    quiet_for += tick;
                    if quiet_for >= IDLE {
                        output = None;
                    }
                } else {
                    quiet_for = Duration::ZERO;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Opens the output for something to play: the first time, and again where
/// its device went away (the sound server restarted, say), but no sooner
/// than [`RETRY`] after it wouldn't open.
fn ready(output: &mut Option<output::Output>, failed_at: &mut Option<Instant>, volume: f32) {
    if output.as_ref().is_none_or(|o| o.stopped()) && failed_at.is_none_or(|at| at.elapsed() >= RETRY) {
        *output = output::Output::open(volume);
        *failed_at = output.is_none().then(Instant::now);
    }
}

/// What the output's callback (or thread) does with each command, between
/// blocks of samples.
#[cfg(any(unix, windows))]
fn apply(mixer: &mut Mixer, cmd: Cmd) {
    match cmd {
        Cmd::Tone(tone) => mixer.play(tone),
        // Its steps go with it; dropped here once it's done, a tune is small.
        Cmd::Tune(tune) => mixer.play_tune(*tune),
        // Its samples are shared with the program, which keeps them: the
        // box alone is dropped here.
        Cmd::Clip(clip) => mixer.play_clip(*clip),
        Cmd::FadeClips(tag, secs) => mixer.fade_clips(tag, secs),
        Cmd::Stop => mixer.stop(),
        Cmd::Volume(v) => mixer.set_volume(v),
        // Made on the output's thread; the one it replaces fading out is
        // dropped here, its voices few and small.
        Cmd::Band(band) => mixer.take_band(band),
        Cmd::StopSong => mixer.stop_song(),
        // Made into a band before it comes here.
        Cmd::Song(song) => mixer.play_song(&song),
    }
}

#[cfg(any(target_os = "macos", windows))]
mod output {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Sender};
    use std::sync::Arc;

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    use super::{Cmd, Mixer};

    /// The system's output, open, the way to its callback, and its samples
    /// a second.
    pub struct Output {
        _stream: cpal::Stream,
        tx: Sender<Cmd>,
        busy: Arc<AtomicBool>,
        pub rate: f32,
    }

    impl Output {
        pub fn open(volume: f32) -> Option<Self> {
            let device = cpal::default_host().default_output_device()?;
            let supported = device.default_output_config().ok()?;
            let (channels, rate) = (supported.channels() as usize, supported.sample_rate() as f32);
            let (tx, rx) = mpsc::channel::<Cmd>();
            let busy = Arc::new(AtomicBool::new(false));
            let mut mixer = Mixer::new(rate);
            mixer.set_volume(volume);
            let config = supported.config();
            let stream = match supported.sample_format() {
                cpal::SampleFormat::F32 => build::<f32>(&device, config, channels, mixer, rx, busy.clone()),
                cpal::SampleFormat::I16 => build::<i16>(&device, config, channels, mixer, rx, busy.clone()),
                cpal::SampleFormat::U16 => build::<u16>(&device, config, channels, mixer, rx, busy.clone()),
                cpal::SampleFormat::I32 => build::<i32>(&device, config, channels, mixer, rx, busy.clone()),
                _ => None,
            }?;
            stream.play().ok()?;
            Some(Self { _stream: stream, tx, busy, rate })
        }

        pub fn send(&self, cmd: Cmd) {
            self.busy.store(true, Ordering::Relaxed);
            let _ = self.tx.send(cmd);
        }

        pub fn busy(&self) -> bool {
            self.busy.load(Ordering::Relaxed)
        }

        /// cpal's stream goes on as long as it's kept.
        pub fn stopped(&self) -> bool {
            false
        }
    }

    fn build<T: cpal::SizedSample + cpal::FromSample<f32>>(
        device: &cpal::Device,
        config: cpal::StreamConfig,
        channels: usize,
        mut mixer: Mixer,
        rx: mpsc::Receiver<Cmd>,
        busy: Arc<AtomicBool>,
    ) -> Option<cpal::Stream> {
        // Room for the usual buffers made now, not in the callback.
        let mut floats: Vec<f32> = Vec::with_capacity(16_384);
        device
            .build_output_stream(
                config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    while let Ok(cmd) = rx.try_recv() {
                        super::apply(&mut mixer, cmd);
                    }
                    floats.resize(data.len(), 0.0);
                    mixer.fill(&mut floats, channels);
                    for (d, s) in data.iter_mut().zip(&floats) {
                        *d = T::from_sample(*s);
                    }
                    busy.store(mixer.busy(), Ordering::Relaxed);
                },
                |_| {},
                None,
            )
            .ok()
    }
}

/// ALSA's default device (Linux), which carries the sound on to PipeWire or
/// PulseAudio where they run: a thread of the output's own mixes a block at
/// a time and writes it, waiting while the device's buffer is full, as
/// cpal's callback is called for more elsewhere.
#[cfg(all(unix, not(target_os = "macos")))]
mod output {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Sender};
    use std::sync::Arc;
    use std::thread::JoinHandle;

    use crate::alsa::Pcm;

    use super::{Cmd, Mixer};

    /// What's asked of the device, which converts to its own as it must.
    const RATE: u32 = 48_000;
    const CHANNELS: usize = 2;
    /// Frames mixed at a time: 10 ms.
    const BLOCK: usize = RATE as usize / 100;

    /// The device, open, the way to its thread, and its samples a second.
    pub struct Output {
        tx: Sender<Cmd>,
        busy: Arc<AtomicBool>,
        open: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
        pub rate: f32,
    }

    impl Output {
        pub fn open(volume: f32) -> Option<Self> {
            let mut pcm = Pcm::open(CHANNELS, RATE).ok()?;
            let (tx, rx) = mpsc::channel::<Cmd>();
            let busy = Arc::new(AtomicBool::new(false));
            let open = Arc::new(AtomicBool::new(true));
            let mut mixer = Mixer::new(RATE as f32);
            mixer.set_volume(volume);
            let (b, o) = (busy.clone(), open.clone());
            let thread = std::thread::Builder::new()
                .name("flow-sdk-audio-out".into())
                .spawn(move || {
                    let mut floats = vec![0.0; BLOCK * CHANNELS];
                    while o.load(Ordering::Relaxed) {
                        while let Ok(cmd) = rx.try_recv() {
                            super::apply(&mut mixer, cmd);
                        }
                        mixer.fill(&mut floats, CHANNELS);
                        if !pcm.write(&floats) {
                            break;
                        }
                        b.store(mixer.busy(), Ordering::Relaxed);
                    }
                    b.store(false, Ordering::Relaxed);
                })
                .ok()?;
            Some(Self { tx, busy, open, thread: Some(thread), rate: RATE as f32 })
        }

        pub fn send(&self, cmd: Cmd) {
            self.busy.store(true, Ordering::Relaxed);
            let _ = self.tx.send(cmd);
        }

        pub fn busy(&self) -> bool {
            self.busy.load(Ordering::Relaxed)
        }

        /// Its thread ended, the device having gone away.
        pub fn stopped(&self) -> bool {
            self.thread.as_ref().is_none_or(|t| t.is_finished())
        }
    }

    impl Drop for Output {
        /// Its thread finishes the block it's writing and closes the device.
        fn drop(&mut self) {
            self.open.store(false, Ordering::Relaxed);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

/// Where there's no output (no OS this is built for), nothing plays.
#[cfg(not(any(unix, windows)))]
mod output {
    use super::Cmd;

    pub struct Output {
        pub rate: f32,
    }

    impl Output {
        pub fn open(_volume: f32) -> Option<Self> {
            None
        }

        pub fn send(&self, _cmd: Cmd) {}

        pub fn busy(&self) -> bool {
            false
        }

        pub fn stopped(&self) -> bool {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn render(mixer: &mut Mixer, secs: f32) -> Vec<f32> {
        let mut out = vec![0.0f32; (RATE * secs) as usize * 2];
        mixer.fill(&mut out, 2);
        out
    }

    fn rms(s: &[f32]) -> f32 {
        (s.iter().map(|x| x * x).sum::<f32>() / s.len().max(1) as f32).sqrt()
    }

    #[test]
    fn a_clip_plays_once_at_its_own_rate_and_then_falls_silent() {
        let mut mixer = Mixer::new(RATE);
        // A tenth of a second at 16 kHz, a steady level, panned left.
        let clip = Clip::new(vec![0.5f32; 1600], 16_000.0).volume(0.8).pan(-1.0);
        assert!((clip.length() - 0.1).abs() < 1e-6);
        mixer.play_clip(clip.clone());
        assert!(mixer.busy());
        let out = render(&mut mixer, 0.15);
        let frame = |i: usize| [out[i * 2], out[i * 2 + 1]];
        let middle = frame((RATE * 0.05) as usize);
        assert!((middle[0] - (0.4f32).tanh()).abs() < 1e-3 && middle[1].abs() < 1e-3, "on the left, at its level: {middle:?}");
        let after = frame((RATE * 0.11) as usize);
        assert_eq!(after, [0.0, 0.0], "a tenth of a second at the output's rate too");
        assert!(!mixer.busy());
        // Shared, not copied: the program's copy plays again.
        mixer.play_clip(clip);
        assert!(rms(&render(&mut mixer, 0.05)) > 0.1);
        mixer.stop();
        assert!(!mixer.busy());
        mixer.play_clip(Clip::new(Vec::new(), 16_000.0));
        assert!(!mixer.busy(), "an empty clip plays nothing");
    }

    #[test]
    fn a_stereo_clip_keeps_its_sides_and_a_loop_plays_round_until_faded() {
        let mut mixer = Mixer::new(RATE);
        // A tenth of a second, the left loud and the right quiet, looped.
        let frames: Vec<f32> = (0..1600).flat_map(|_| [0.5f32, 0.1]).collect();
        let music = Clip::stereo(frames, 16_000.0).looped().tag(7);
        assert!((music.length() - 0.1).abs() < 1e-6, "a frame a pair");
        mixer.play_clip(music);
        let out = render(&mut mixer, 0.35);
        let frame = |i: usize| [out[i * 2], out[i * 2 + 1]];
        for t in [0.05, 0.15, 0.32] {
            let [l, r] = frame((RATE * t) as usize);
            assert!((l - 0.5f32.tanh()).abs() < 1e-3 && (r - 0.1f32.tanh()).abs() < 1e-3, "round and round, each side its own, at {t}: {l} {r}");
        }
        // A voice beside it, untagged, plays on as the music fades.
        mixer.play_clip(Clip::new(vec![0.3f32; 16_000], 16_000.0).pan(1.0));
        mixer.fade_clips(7, 0.1);
        let out = render(&mut mixer, 0.2);
        let [l, r] = [out[(RATE * 0.15) as usize * 2], out[(RATE * 0.15) as usize * 2 + 1]];
        assert!(l.abs() < 1e-3 && r > 0.2, "the music faded, the voice on: {l} {r}");
        assert!(mixer.busy());
        mixer.fade_clips(0, 0.0);
        assert!(!mixer.busy(), "faded at once");
    }

    #[test]
    fn a_clip_may_come_up_from_silence() {
        let mut mixer = Mixer::new(RATE);
        mixer.play_clip(Clip::new(vec![0.5f32; 16_000], 16_000.0).fade_in(0.5));
        let out = render(&mut mixer, 0.8);
        let at = |t: f32| out[(RATE * t) as usize * 2];
        assert!(at(0.05) < at(0.25) && at(0.25) < at(0.45) && (at(0.7) - at(0.6)).abs() < 1e-4, "{} {} {} {}", at(0.05), at(0.25), at(0.45), at(0.7));
    }

    #[test]
    fn notes_are_named_as_written() {
        assert!((pitch("A4").unwrap() - 440.0).abs() < 0.01);
        assert!((pitch("C4").unwrap() - 261.63).abs() < 0.01);
        assert!((pitch("Bb3").unwrap() - 233.08).abs() < 0.01);
        assert!((pitch("F#4").unwrap() - 369.99).abs() < 0.01);
        assert_eq!(pitch("H4"), None);
        assert!(Tune::parse("A4", 120.0).is_err(), "a note needs its length");
        assert!(Tune::parse("A4/x", 120.0).is_err());
    }

    #[test]
    fn a_tone_sounds_never_clipping_and_then_falls_silent() {
        for wave in [Wave::Sine, Wave::Triangle, Wave::Square, Wave::Saw, Wave::Noise] {
            let mut mixer = Mixer::new(RATE);
            mixer.play(Tone::new(440.0, 0.2).wave(wave).volume(1.0).pan(0.5));
            let out = render(&mut mixer, 0.2);
            let peak = out.iter().fold(0.0f32, |a, s| a.max(s.abs()));
            assert!(out.iter().all(|s| s.is_finite()) && peak < 1.0 && rms(&out) > 0.1, "{wave:?}: peak {peak}");
            // Panned right, it's louder there.
            assert!(rms(&out.iter().skip(1).step_by(2).copied().collect::<Vec<_>>()) > rms(&out.iter().step_by(2).copied().collect::<Vec<_>>()));
            render(&mut mixer, 0.1);
            assert!(!mixer.busy());
            assert!(render(&mut mixer, 0.05).iter().all(|s| *s == 0.0));
        }
    }

    #[test]
    fn a_tunes_notes_come_on_the_beat_and_a_looped_one_goes_round() {
        // Two beats a second: a note, a rest, a note.
        let tune = Tune::parse("A4/1 r/1 A4/1", 120.0).unwrap().sound(Tone::new(0.0, 0.0).envelope(0.001, 0.01));
        assert!((tune.length() - 1.5).abs() < 1e-6);
        let mut mixer = Mixer::new(RATE);
        mixer.play_tune(tune.clone());
        let out = render(&mut mixer, 1.5);
        let at = |t: f32| rms(&out[(t * RATE) as usize * 2..((t + 0.1) * RATE) as usize * 2]);
        assert!(at(0.1) > 0.05 && at(0.7) < 1e-3 && at(1.1) > 0.05, "{} {} {}", at(0.1), at(0.7), at(1.1));
        render(&mut mixer, 0.2);
        assert!(!mixer.busy(), "once through, it's done");

        let mut looped = Mixer::new(RATE);
        looped.play_tune(tune.looped());
        render(&mut looped, 3.0);
        assert!(looped.busy() && rms(&render(&mut looped, 0.2)) > 0.05, "round again");
        looped.stop();
        assert!(render(&mut looped, 0.1).iter().all(|s| *s == 0.0));
    }

    #[test]
    fn the_volume_scales_it_all_and_zero_is_silence() {
        let mut mixer = Mixer::new(RATE);
        mixer.set_volume(0.0);
        mixer.play(Tone::new(440.0, 0.1));
        assert!(render(&mut mixer, 0.1).iter().all(|s| *s == 0.0));
    }

    /// A dance for the whole band, eight beats long.
    fn band() -> Song {
        Song::new(
            330.0,
            vec![
                Line::parse(Instrument::Lute, 1.0, "A4/3 G4/1 F4/2 D5/2").unwrap(),
                Line::parse(Instrument::Recorder, 0.9, "D5/4 C5/2 A4/2").unwrap(),
                Line::parse(Instrument::Drone, 1.0, "D3/8").unwrap(),
                Line::parse(Instrument::Tabor, 0.7, "X/3 x/2 x/1 r/2").unwrap(),
            ],
        )
    }

    #[test]
    fn a_songs_lines_read_as_written_and_the_tabor_by_how_hard() {
        let drum = Line::parse(Instrument::Tabor, 1.0, "X/2 x/1 r/1").unwrap();
        assert_eq!(drum.steps, [Step { hz: Some(1.0), beats: 2.0 }, Step { hz: Some(0.55), beats: 1.0 }, Step { hz: None, beats: 1.0 }]);
        assert!(Line::parse(Instrument::Lute, 1.0, "X/2").is_err(), "a beat is the tabor's alone");
        assert!(Line::parse(Instrument::Lute, 1.0, "A4").is_err(), "a note needs its length");
        let song = band();
        assert!(song.lines.iter().all(|l| l.beats() == 8.0));
        assert!((song.length() - 8.0 * 60.0 / 330.0).abs() < 1e-6);
    }

    #[test]
    fn every_instrument_plays_heard_and_never_clipping() {
        for instrument in Instrument::ALL {
            let notes = if instrument == Instrument::Tabor { "X/2 x/2" } else { "D3/2 A4/2" };
            let mut mixer = Mixer::new(RATE);
            mixer.set_volume(0.7);
            mixer.play_song(&Song::new(120.0, vec![Line::parse(instrument, 1.0, notes).unwrap()]));
            let out = render(&mut mixer, 3.0);
            let peak = out.iter().fold(0.0f32, |a, s| a.max(s.abs()));
            assert!(out.iter().all(|s| s.is_finite()) && peak < 1.0 && rms(&out) > 0.005, "{instrument:?}: peak {peak}, rms {}", rms(&out));
        }
        let mut mixer = Mixer::new(RATE);
        mixer.play_song(&band());
        let out = render(&mut mixer, 4.0);
        assert!(out.iter().fold(0.0f32, |a, s| a.max(s.abs())) < 1.0 && rms(&out) > 0.02);
    }

    #[test]
    fn a_song_fades_into_the_next_and_out_when_stopped() {
        let mut mixer = Mixer::new(RATE);
        mixer.play_song(&band());
        let first = render(&mut mixer, 1.0);
        assert!(rms(&first[..4800]) < rms(&first[first.len() - 9600..]), "fading in");
        mixer.play_song(&Song::new(200.0, vec![Line::parse(Instrument::Recorder, 1.0, "E5/2 G5/2").unwrap()]));
        assert!(mixer.leaving.is_some());
        render(&mut mixer, 1.3);
        assert!(mixer.leaving.is_none(), "the first is gone once faded");
        assert!(mixer.busy() && rms(&render(&mut mixer, 0.5)) > 0.01, "the second loops on");
        mixer.stop_song();
        render(&mut mixer, 1.3);
        assert!(!mixer.busy());
        assert!(render(&mut mixer, 0.1).iter().all(|s| s.abs() < 1e-6), "silent");
        // Stopping everything takes a song at once.
        mixer.play_song(&band());
        render(&mut mixer, 0.5);
        mixer.stop();
        assert!(!mixer.busy() && render(&mut mixer, 0.1).iter().all(|s| *s == 0.0));
    }
}
