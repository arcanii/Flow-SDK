//! The program's own audio from C ([`flow_sdk::audio`], behind this
//! crate's `audio` feature, on by default): tones, tunes written as note
//! names, and songs for a small band of instruments made in code (a
//! plucked lute, a recorder, a drone and a tabor), looped and faded one
//! into the next. A `FlowAudio` plays them on the machine, its thread
//! opening the system's output with the first sound (CoreAudio, WASAPI,
//! or ALSA's default device on Linux); a `FlowMixer` mixes them into the caller's
//! own buffer, for an output of the program's own, or none.
//!
//! Every function is exported whether or not the library was built with
//! the feature, so one header and one interface serve both builds: without
//! it each refuses (NULL, -1, 0 or false), and [`flow_audio_built`] says
//! so, for a program (or Python) to go on without music.
#![cfg_attr(not(feature = "audio"), allow(unused_variables, unused_imports, dead_code))]

use std::ffi::{c_char, c_int};

#[cfg(feature = "audio")]
use flow_sdk::audio::{self, Audio, Instrument, Line, Mixer, Song, Tone, Tune, Wave};

use crate::{given, guard, tell, text, untold};

/// Runs `body` under [`guard`] where the library was built with audio, and
/// gives `fallback` where it wasn't, without compiling `body` at all.
macro_rules! with_audio {
    ($fallback:expr, $body:expr) => {{
        #[cfg(feature = "audio")]
        let done = guard($fallback, $body);
        #[cfg(not(feature = "audio"))]
        let done = $fallback;
        done
    }};
}

/// `FLOW_WAVE_*`: a tone's shape.
pub const WAVE_SINE: i32 = 0;
pub const WAVE_TRIANGLE: i32 = 1;
pub const WAVE_SQUARE: i32 = 2;
pub const WAVE_SAW: i32 = 3;
pub const WAVE_NOISE: i32 = 4;

/// `FLOW_INSTRUMENT_*`: the place in the SDK's `Instrument::ALL`.
pub const INSTRUMENT_LUTE: i32 = 0;
pub const INSTRUMENT_RECORDER: i32 = 1;
pub const INSTRUMENT_DRONE: i32 = 2;
pub const INSTRUMENT_TABOR: i32 = 3;

/// What a library built without audio says when asked for it.
#[cfg(not(feature = "audio"))]
const WITHOUT: &str = "this build of Flow's C library has no audio (it was built without its `audio` feature)";

/// The waves in `FLOW_WAVE_*` order.
#[cfg(feature = "audio")]
const WAVES: [Wave; 5] = [Wave::Sine, Wave::Triangle, Wave::Square, Wave::Saw, Wave::Noise];

/// One note: its pitch and how long it's held, how it sounds, and how it
/// comes in and dies away ([`flow_sdk::audio::Tone`]).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FlowTone {
    pub hz: f32,
    pub secs: f32,
    /// `FLOW_WAVE_*`.
    pub wave: i32,
    /// 0 to 1.
    pub volume: f32,
    /// Left (-1) to right (1).
    pub pan: f32,
    /// Seconds to come in, and to die away after it's held.
    pub attack: f32,
    pub release: f32,
}

#[cfg(feature = "audio")]
impl FlowTone {
    pub fn of(tone: Tone) -> Self {
        let wave = WAVES.iter().position(|w| *w == tone.wave).map_or(WAVE_SINE, |i| i as i32);
        Self { hz: tone.hz, secs: tone.secs, wave, volume: tone.volume, pan: tone.pan, attack: tone.attack, release: tone.release }
    }

    /// The SDK's tone, if its wave is one there is and every number is
    /// finite: one that isn't would fill the mix with NAN.
    pub fn tone(&self) -> Option<Tone> {
        let wave = *WAVES.get(usize::try_from(self.wave).ok()?)?;
        let numbers = [self.hz, self.secs, self.volume, self.pan, self.attack, self.release];
        numbers.iter().all(|v| v.is_finite()).then_some(Tone { hz: self.hz, secs: self.secs, wave, volume: self.volume, pan: self.pan, attack: self.attack, release: self.release })
    }
}

/// A tune being kept: `flow_tune_new` to `flow_tune_free`.
pub struct FlowTune(#[cfg(feature = "audio")] pub(crate) Tune);

/// A song being built: `flow_song_new` to `flow_song_free`.
pub struct FlowSong(#[cfg(feature = "audio")] pub(crate) Song);

/// The program's sound on the machine: `flow_audio_new` to
/// `flow_audio_free`.
pub struct FlowAudio(#[cfg(feature = "audio")] pub(crate) Audio);

/// A mix rendered into the caller's buffers: `flow_mixer_new` to
/// `flow_mixer_free`.
pub struct FlowMixer(#[cfg(feature = "audio")] pub(crate) Mixer);

#[cfg(feature = "audio")]
unsafe fn tone_at(tone: *const FlowTone) -> Option<Tone> {
    tone.as_ref()?.tone()
}

#[cfg(feature = "audio")]
fn instrument(n: i32) -> Option<Instrument> {
    Instrument::ALL.get(usize::try_from(n).ok()?).copied()
}

/// A tempo or a rate: finite and above 0.
fn positive(value: f32) -> Option<f32> {
    given(value).filter(|v| *v > 0.0)
}

/// Does `act` to the handle at `handle`: 0, or -1 for NULL or a value
/// `act` refused.
unsafe fn on<T>(handle: *mut T, act: impl FnOnce(&mut T) -> Option<()>) -> c_int {
    handle.as_mut().and_then(act).map_or(-1, |()| 0)
}

/// Frees a handle made by `Box::into_raw`; NULL is let be.
unsafe fn free<T>(handle: *mut T) {
    if !handle.is_null() {
        guard((), || drop(Box::from_raw(handle)));
    }
}

/// Whether this build of the library has audio: its `audio` feature, on
/// unless it was built with `--no-default-features`.
#[no_mangle]
pub extern "C" fn flow_audio_built() -> bool {
    cfg!(feature = "audio")
}

/// Whether a `FlowAudio` plays anything here: a build with audio, on macOS,
/// Windows or Linux.
#[no_mangle]
pub extern "C" fn flow_audio_plays() -> bool {
    with_audio!(false, || audio::AVAILABLE)
}

/// A note's pitch in hertz from its name (`A4` 440, `C#5`, `Bb3`); 0 for
/// a name that's no note.
#[no_mangle]
pub unsafe extern "C" fn flow_pitch(name: *const c_char) -> f32 {
    with_audio!(0.0, || text(name).and_then(audio::pitch).unwrap_or(0.0))
}

/// A tone of `hz` held `secs` as the SDK makes one: a sine at half volume
/// in the middle, coming in over 5 ms and dying away over 80.
#[no_mangle]
pub unsafe extern "C" fn flow_tone_init(tone: *mut FlowTone, hz: f32, secs: f32) -> c_int {
    with_audio!(-1, || on(tone, |t| {
        *t = FlowTone::of(Tone::new(hz, secs));
        Some(())
    }))
}

/// The tune for [`flow_tune_new`], or why there's none.
unsafe fn tune(notes: *const c_char, tempo: f32, sound: *const FlowTone, looped: bool) -> Result<FlowTune, String> {
    #[cfg(not(feature = "audio"))]
    return Err(WITHOUT.to_string());
    #[cfg(feature = "audio")]
    {
        let notes = text(notes).ok_or("the notes are NULL, or aren't UTF-8")?;
        let tempo = positive(tempo).ok_or("a tempo is beats a minute, above 0")?;
        let mut tune = Tune::parse(notes, tempo)?;
        if !sound.is_null() {
            tune = tune.sound(tone_at(sound).ok_or("the tone's wave is none there is, or a number of it isn't finite")?);
        }
        Ok(FlowTune(if looped { tune.looped() } else { tune }))
    }
}

/// A tune from notes written as `name/beats` (`"C4/1 E4/1 G4/2 r/1"`, `r`
/// a rest) at `tempo` beats a minute, each note sounding as `sound` does
/// (NULL for the SDK's tone), played once or `looped`.
#[no_mangle]
pub unsafe extern "C" fn flow_tune_new(notes: *const c_char, tempo: f32, sound: *const FlowTone, looped: bool, error: *mut *mut c_char) -> *mut FlowTune {
    guard(std::ptr::null_mut(), || {
        untold(error);
        match tune(notes, tempo, sound, looped) {
            Ok(tune) => Box::into_raw(Box::new(tune)),
            Err(why) => {
                tell(error, &why);
                std::ptr::null_mut()
            }
        }
    })
}

/// Seconds once through.
#[no_mangle]
pub unsafe extern "C" fn flow_tune_length(tune: *const FlowTune) -> f32 {
    with_audio!(0.0, || tune.as_ref().map_or(0.0, |t| t.0.length()))
}

#[no_mangle]
pub unsafe extern "C" fn flow_tune_free(tune: *mut FlowTune) {
    free(tune);
}

/// A song with no lines yet, at `tempo` beats a minute.
#[no_mangle]
pub extern "C" fn flow_song_new(tempo: f32) -> *mut FlowSong {
    with_audio!(std::ptr::null_mut(), || match positive(tempo) {
        Some(tempo) => Box::into_raw(Box::new(FlowSong(Song::new(tempo, Vec::new())))),
        None => std::ptr::null_mut(),
    })
}

/// The line for [`flow_song_line`], added, or why it isn't.
unsafe fn line(song: *mut FlowSong, instrument: i32, gain: f32, notes: *const c_char) -> Result<(), String> {
    #[cfg(not(feature = "audio"))]
    return Err(WITHOUT.to_string());
    #[cfg(feature = "audio")]
    {
        let song = song.as_mut().ok_or("the song is NULL")?;
        let instrument = self::instrument(instrument).ok_or(format!("there's no instrument {instrument}"))?;
        let gain = given(gain).ok_or("the gain isn't a finite number")?;
        let notes = text(notes).ok_or("the notes are NULL, or aren't UTF-8")?;
        song.0.lines.push(Line::parse(instrument, gain, notes)?);
        Ok(())
    }
}

/// Adds a line to the song: `instrument` (`FLOW_INSTRUMENT_*`) playing
/// `notes`, written as a tune's are (`D4/3`, `Bb3/2`, `r/1`; on the tabor
/// `X/1` a full beat and `x/1` a light one), as loud as `gain` against the
/// instrument's own level.
#[no_mangle]
pub unsafe extern "C" fn flow_song_line(song: *mut FlowSong, instrument: i32, gain: f32, notes: *const c_char, error: *mut *mut c_char) -> c_int {
    guard(-1, || {
        untold(error);
        match line(song, instrument, gain, notes) {
            Ok(()) => 0,
            Err(why) => {
                tell(error, &why);
                -1
            }
        }
    })
}

/// Seconds once through: its longest line's beats.
#[no_mangle]
pub unsafe extern "C" fn flow_song_length(song: *const FlowSong) -> f32 {
    with_audio!(0.0, || song.as_ref().map_or(0.0, |s| s.0.length()))
}

#[no_mangle]
pub unsafe extern "C" fn flow_song_free(song: *mut FlowSong) {
    free(song);
}

/// The program's sound, which opens nothing until the first sound.
#[no_mangle]
pub extern "C" fn flow_audio_new() -> *mut FlowAudio {
    with_audio!(std::ptr::null_mut(), || Box::into_raw(Box::new(FlowAudio(Audio::new()))))
}

/// Ends the program's sound: what's playing stops, and the output closes.
#[no_mangle]
pub unsafe extern "C" fn flow_audio_free(audio: *mut FlowAudio) {
    free(audio);
}

#[no_mangle]
pub unsafe extern "C" fn flow_audio_play(audio: *mut FlowAudio, tone: *const FlowTone) -> c_int {
    with_audio!(-1, || on(audio, |a| {
        a.0.play(tone_at(tone)?);
        Some(())
    }))
}

#[no_mangle]
pub unsafe extern "C" fn flow_audio_play_tune(audio: *mut FlowAudio, tune: *const FlowTune) -> c_int {
    with_audio!(-1, || on(audio, |a| {
        a.0.play_tune(tune.as_ref()?.0.clone());
        Some(())
    }))
}

#[no_mangle]
pub unsafe extern "C" fn flow_audio_play_song(audio: *mut FlowAudio, song: *const FlowSong) -> c_int {
    with_audio!(-1, || on(audio, |a| {
        a.0.play_song(song.as_ref()?.0.clone());
        Some(())
    }))
}

#[no_mangle]
pub unsafe extern "C" fn flow_audio_stop_song(audio: *mut FlowAudio) -> c_int {
    with_audio!(-1, || on(audio, |a| {
        a.0.stop_song();
        Some(())
    }))
}

#[no_mangle]
pub unsafe extern "C" fn flow_audio_stop(audio: *mut FlowAudio) -> c_int {
    with_audio!(-1, || on(audio, |a| {
        a.0.stop();
        Some(())
    }))
}

#[no_mangle]
pub unsafe extern "C" fn flow_audio_set_volume(audio: *mut FlowAudio, volume: f32) -> c_int {
    with_audio!(-1, || on(audio, |a| {
        a.0.set_volume(given(volume)?);
        Some(())
    }))
}

/// A mix at `rate` samples a second.
#[no_mangle]
pub extern "C" fn flow_mixer_new(rate: f32) -> *mut FlowMixer {
    with_audio!(std::ptr::null_mut(), || match positive(rate) {
        Some(rate) => Box::into_raw(Box::new(FlowMixer(Mixer::new(rate)))),
        None => std::ptr::null_mut(),
    })
}

#[no_mangle]
pub unsafe extern "C" fn flow_mixer_free(mixer: *mut FlowMixer) {
    free(mixer);
}

#[no_mangle]
pub unsafe extern "C" fn flow_mixer_play(mixer: *mut FlowMixer, tone: *const FlowTone) -> c_int {
    with_audio!(-1, || on(mixer, |m| {
        m.0.play(tone_at(tone)?);
        Some(())
    }))
}

#[no_mangle]
pub unsafe extern "C" fn flow_mixer_play_tune(mixer: *mut FlowMixer, tune: *const FlowTune) -> c_int {
    with_audio!(-1, || on(mixer, |m| {
        m.0.play_tune(tune.as_ref()?.0.clone());
        Some(())
    }))
}

#[no_mangle]
pub unsafe extern "C" fn flow_mixer_play_song(mixer: *mut FlowMixer, song: *const FlowSong) -> c_int {
    with_audio!(-1, || on(mixer, |m| {
        m.0.play_song(&song.as_ref()?.0);
        Some(())
    }))
}

#[no_mangle]
pub unsafe extern "C" fn flow_mixer_stop_song(mixer: *mut FlowMixer) -> c_int {
    with_audio!(-1, || on(mixer, |m| {
        m.0.stop_song();
        Some(())
    }))
}

#[no_mangle]
pub unsafe extern "C" fn flow_mixer_stop(mixer: *mut FlowMixer) -> c_int {
    with_audio!(-1, || on(mixer, |m| {
        m.0.stop();
        Some(())
    }))
}

#[no_mangle]
pub unsafe extern "C" fn flow_mixer_set_volume(mixer: *mut FlowMixer, volume: f32) -> c_int {
    with_audio!(-1, || on(mixer, |m| {
        m.0.set_volume(given(volume)?);
        Some(())
    }))
}

/// Whether anything is still sounding or still to come: a song, until it
/// has faded out.
#[no_mangle]
pub unsafe extern "C" fn flow_mixer_busy(mixer: *const FlowMixer) -> bool {
    with_audio!(false, || mixer.as_ref().is_some_and(|m| m.0.busy()))
}

/// Fills `len` floats at `out` with the mix, `channels` a frame (left and
/// right first, any more silent, one alone both).
#[no_mangle]
pub unsafe extern "C" fn flow_mixer_fill(mixer: *mut FlowMixer, out: *mut f32, len: usize, channels: u32) -> c_int {
    with_audio!(-1, || {
        let Some(mixer) = mixer.as_mut() else { return -1 };
        if channels == 0 || (out.is_null() && len > 0) {
            return -1;
        }
        let out: &mut [f32] = if len == 0 { &mut [] } else { std::slice::from_raw_parts_mut(out, len) };
        mixer.0.fill(out, channels as usize);
        0
    })
}

#[cfg(all(test, feature = "audio"))]
mod tests {
    use super::*;
    use std::ffi::CStr;

    const RATE: f32 = 48_000.0;

    fn rendered(mixer: *mut FlowMixer, secs: f32) -> Vec<f32> {
        let mut out = vec![0.0f32; (RATE * secs) as usize * 2];
        assert_eq!(unsafe { flow_mixer_fill(mixer, out.as_mut_ptr(), out.len(), 2) }, 0);
        out
    }

    fn rust_rendered(mixer: &mut Mixer, secs: f32) -> Vec<f32> {
        let mut out = vec![0.0f32; (RATE * secs) as usize * 2];
        mixer.fill(&mut out, 2);
        out
    }

    fn rms(s: &[f32]) -> f32 {
        (s.iter().map(|x| x * x).sum::<f32>() / s.len().max(1) as f32).sqrt()
    }

    /// The reason left in `error`, freed.
    fn reason(error: *mut c_char) -> String {
        assert!(!error.is_null(), "a reason is given");
        // SAFETY: handed out by this crate, freed once.
        unsafe {
            let text = CStr::from_ptr(error).to_str().unwrap().to_string();
            crate::flow_free(error.cast());
            text
        }
    }

    /// A dance for the whole band, eight beats long, as the SDK's tests have it.
    const BAND: [(i32, Instrument, f32, &str); 4] = [
        (INSTRUMENT_LUTE, Instrument::Lute, 1.0, "A4/3 G4/1 F4/2 D5/2"),
        (INSTRUMENT_RECORDER, Instrument::Recorder, 0.9, "D5/4 C5/2 A4/2"),
        (INSTRUMENT_DRONE, Instrument::Drone, 1.0, "D3/8"),
        (INSTRUMENT_TABOR, Instrument::Tabor, 0.7, "X/3 x/2 x/1 r/2"),
    ];

    #[test]
    fn the_numbers_name_the_sdks_waves_and_instruments() {
        let waves = [(WAVE_SINE, Wave::Sine), (WAVE_TRIANGLE, Wave::Triangle), (WAVE_SQUARE, Wave::Square), (WAVE_SAW, Wave::Saw), (WAVE_NOISE, Wave::Noise)];
        for (n, wave) in waves {
            let tone = Tone::new(220.0, 1.0).wave(wave);
            assert_eq!((FlowTone::of(tone).wave, FlowTone { wave: n, ..FlowTone::of(tone) }.tone()), (n, Some(tone)));
        }
        for (n, rust, _, _) in BAND {
            assert_eq!(instrument(n), Some(rust));
        }
        assert_eq!(Instrument::ALL.len(), 4);
        assert_eq!(instrument(4), None);
    }

    #[test]
    fn a_tone_from_c_is_the_sdks_and_mixes_sample_for_sample_as_it_does() {
        let mut tone = FlowTone::default();
        unsafe { assert_eq!(flow_tone_init(&mut tone, 440.0, 0.2), 0) };
        assert_eq!(tone.tone(), Some(Tone::new(440.0, 0.2)));
        (tone.wave, tone.pan, tone.volume) = (WAVE_SQUARE, 0.5, 1.0);
        let mixer = flow_mixer_new(RATE);
        let mut rust = Mixer::new(RATE);
        rust.play(Tone::new(440.0, 0.2).wave(Wave::Square).pan(0.5).volume(1.0));
        unsafe { assert_eq!(flow_mixer_play(mixer, &tone), 0) };
        assert_eq!(rendered(mixer, 0.4), rust_rendered(&mut rust, 0.4));
        assert!(!unsafe { flow_mixer_busy(mixer) }, "it has died away");

        let sound = FlowTone { wave: WAVE_TRIANGLE, release: 0.01, ..tone };
        let tune = unsafe { flow_tune_new(c"A4/1 r/1 A4/1".as_ptr(), 120.0, &sound, true, std::ptr::null_mut()) };
        assert!((unsafe { flow_tune_length(tune) } - 1.5).abs() < 1e-6);
        rust.play_tune(Tune::parse("A4/1 r/1 A4/1", 120.0).unwrap().sound(sound.tone().unwrap()).looped());
        unsafe { assert_eq!(flow_mixer_play_tune(mixer, tune), 0) };
        assert_eq!(rendered(mixer, 3.2), rust_rendered(&mut rust, 3.2), "round again, as in Rust");
        unsafe {
            assert_eq!(flow_mixer_stop(mixer), 0);
            assert!(!flow_mixer_busy(mixer));
            flow_tune_free(tune);
            flow_mixer_free(mixer);
        }
    }

    #[test]
    fn a_song_built_from_c_mixes_as_the_sdks_and_fades_into_the_next() {
        let song = flow_song_new(330.0);
        for (n, _, gain, notes) in BAND {
            let notes = std::ffi::CString::new(notes).unwrap();
            assert_eq!(unsafe { flow_song_line(song, n, gain, notes.as_ptr(), std::ptr::null_mut()) }, 0);
        }
        let rust_song = Song::new(330.0, BAND.iter().map(|&(_, i, gain, notes)| Line::parse(i, gain, notes).unwrap()).collect());
        assert_eq!(unsafe { &(*song).0 }, &rust_song);
        assert!((unsafe { flow_song_length(song) } - rust_song.length()).abs() < 1e-6);
        let next = flow_song_new(200.0);
        unsafe { assert_eq!(flow_song_line(next, INSTRUMENT_RECORDER, 1.0, c"E5/2 G5/2".as_ptr(), std::ptr::null_mut()), 0) };

        let mixer = flow_mixer_new(RATE);
        let mut rust = Mixer::new(RATE);
        unsafe {
            assert_eq!(flow_mixer_set_volume(mixer, 0.8), 0);
            assert_eq!(flow_mixer_play_song(mixer, song), 0);
        }
        rust.set_volume(0.8);
        rust.play_song(&rust_song);
        let first = rendered(mixer, 1.0);
        assert_eq!(first, rust_rendered(&mut rust, 1.0));
        assert!(rms(&first[..4800]) < rms(&first[first.len() - 9600..]), "fading in");
        unsafe { assert_eq!(flow_mixer_play_song(mixer, next), 0) };
        rust.play_song(&Song::new(200.0, vec![Line::parse(Instrument::Recorder, 1.0, "E5/2 G5/2").unwrap()]));
        assert_eq!(rendered(mixer, 1.5), rust_rendered(&mut rust, 1.5), "the first fading under the next");
        unsafe { assert_eq!(flow_mixer_stop_song(mixer), 0) };
        rust.stop_song();
        assert_eq!(rendered(mixer, 1.3), rust_rendered(&mut rust, 1.3));
        assert!(!unsafe { flow_mixer_busy(mixer) }, "faded out");
        unsafe {
            flow_mixer_free(mixer);
            flow_song_free(song);
            flow_song_free(next);
        }
    }

    #[test]
    fn notes_that_dont_read_are_refused_with_the_reason() {
        let song = flow_song_new(120.0);
        let mut error: *mut c_char = std::ptr::null_mut();
        unsafe {
            assert_eq!(flow_song_line(song, INSTRUMENT_LUTE, 1.0, c"A4".as_ptr(), &mut error), -1);
            assert_eq!(reason(error), "A4: a note needs its length");
            assert_eq!(flow_song_line(song, INSTRUMENT_LUTE, 1.0, c"X/2".as_ptr(), &mut error), -1, "a beat is the tabor's alone");
            assert_eq!(reason(error), "X/2: no such note");
            assert_eq!(flow_song_line(song, 9, 1.0, c"A4/1".as_ptr(), &mut error), -1);
            assert_eq!(reason(error), "there's no instrument 9");
            assert_eq!(flow_song_line(song, INSTRUMENT_TABOR, 1.0, c"X/1".as_ptr(), &mut error), 0);
            assert!(error.is_null(), "no reason when it reads");
            assert_eq!(flow_song_line(song, INSTRUMENT_LUTE, f32::NAN, c"A4/1".as_ptr(), std::ptr::null_mut()), -1, "no reason wanted");
            assert_eq!(flow_song_line(std::ptr::null_mut(), INSTRUMENT_LUTE, 1.0, c"A4/1".as_ptr(), &mut error), -1);
            assert_eq!(reason(error), "the song is NULL");
            assert!(flow_tune_new(c"A4/x".as_ptr(), 120.0, std::ptr::null(), false, &mut error).is_null());
            assert_eq!(reason(error), "A4/x: its length isn't a number");
            assert!(flow_tune_new(c"A4/1".as_ptr(), 0.0, std::ptr::null(), false, &mut error).is_null());
            assert!(reason(error).contains("tempo"));
            let bad = FlowTone { wave: 9, ..FlowTone::default() };
            assert!(flow_tune_new(c"A4/1".as_ptr(), 120.0, &bad, false, &mut error).is_null());
            assert!(reason(error).contains("wave"));
            flow_song_free(song);
        }
    }

    #[test]
    fn what_cant_be_played_is_refused_and_the_player_opens_nothing_unasked() {
        unsafe {
            assert!((flow_pitch(c"A4".as_ptr()) - 440.0).abs() < 0.01);
            assert_eq!((flow_pitch(c"H4".as_ptr()), flow_pitch(std::ptr::null())), (0.0, 0.0));
        }
        assert!(flow_audio_built());
        assert_eq!(flow_audio_plays(), audio::AVAILABLE);
        assert!(flow_song_new(f32::NAN).is_null() && flow_song_new(0.0).is_null());
        assert!(flow_mixer_new(0.0).is_null());
        let mixer = flow_mixer_new(RATE);
        let mut tone = FlowTone::default();
        let mut out = [1.0f32; 4];
        unsafe {
            flow_tone_init(&mut tone, 440.0, 0.1);
            assert_eq!(flow_mixer_play(mixer, &FlowTone { hz: f32::NAN, ..tone }), -1);
            assert_eq!(flow_mixer_play(mixer, &FlowTone { wave: 5, ..tone }), -1);
            assert_eq!(flow_mixer_play(mixer, std::ptr::null()), -1);
            assert_eq!(flow_mixer_play_song(mixer, std::ptr::null()), -1);
            assert_eq!(flow_mixer_set_volume(mixer, f32::NAN), -1);
            assert_eq!(flow_mixer_fill(mixer, std::ptr::null_mut(), 4, 2), -1);
            assert_eq!(flow_mixer_fill(mixer, out.as_mut_ptr(), 4, 0), -1);
            assert_eq!(flow_mixer_fill(mixer, std::ptr::null_mut(), 0, 2), 0, "nothing to fill");
            assert_eq!(flow_mixer_fill(mixer, out.as_mut_ptr(), 4, 2), 0);
            assert_eq!(out, [0.0; 4], "silence, with nothing playing");
            assert_eq!(flow_mixer_stop(std::ptr::null_mut()), -1);
            assert!(!flow_mixer_busy(std::ptr::null()));
            flow_mixer_free(mixer);
            // Stopping before anything's played starts no thread and opens
            // no output; playing would, so the tests don't.
            let audio = flow_audio_new();
            assert!(!audio.is_null());
            assert_eq!(flow_audio_stop_song(audio) | flow_audio_stop(audio), 0);
            assert_eq!(flow_audio_play(audio, &FlowTone { wave: 5, ..tone }), -1, "refused before it reaches the thread");
            assert_eq!(flow_audio_play(std::ptr::null_mut(), &tone), -1);
            flow_audio_free(audio);
            flow_audio_free(std::ptr::null_mut());
            flow_tune_free(std::ptr::null_mut());
            flow_song_free(std::ptr::null_mut());
        }
    }
}

#[cfg(all(test, not(feature = "audio")))]
mod without {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn without_audio_everything_refuses_and_says_why() {
        assert!(!flow_audio_built() && !flow_audio_plays());
        let mut error: *mut c_char = std::ptr::null_mut();
        let mut tone = FlowTone::default();
        unsafe {
            assert_eq!(flow_pitch(c"A4".as_ptr()), 0.0);
            assert_eq!(flow_tone_init(&mut tone, 440.0, 0.1), -1);
            assert!(flow_tune_new(c"A4/1".as_ptr(), 120.0, std::ptr::null(), false, &mut error).is_null());
            assert_eq!(CStr::from_ptr(error).to_str().unwrap(), WITHOUT);
            crate::flow_free(error.cast());
            assert_eq!(flow_song_line(std::ptr::null_mut(), INSTRUMENT_LUTE, 1.0, c"A4/1".as_ptr(), std::ptr::null_mut()), -1);
            assert_eq!(flow_mixer_fill(std::ptr::null_mut(), std::ptr::null_mut(), 0, 2), -1);
        }
        assert!(flow_song_new(120.0).is_null() && flow_mixer_new(48_000.0).is_null() && flow_audio_new().is_null());
    }
}
