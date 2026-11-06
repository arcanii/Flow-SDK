//! Sound out through ALSA, on Linux and the other Unixes but macOS, with
//! ALSA's library loaded as the program runs (`libasound.so.2`, which
//! desktops have, and whose PipeWire and PulseAudio plugins carry the sound
//! on to the sound server), as winit and wgpu load the system's libraries:
//! building needs no headers, and where there's no library or no device,
//! nothing plays rather than the program not starting. A [`Pcm`] is the
//! default device, open for interleaved samples, written a block at a time
//! from a thread of the player's own; [`Pcm::write`] waits while the
//! device's buffer is full, which paces that thread. It needs nothing but
//! std and `libc` (Flow keeps a copy of its own for its own sounds).

use std::ffi::{c_char, c_int, c_long, c_uint, c_ulong, c_void, CStr};
use std::sync::OnceLock;

/// ALSA's numbers for what's asked of it (alsa/pcm.h): samples in this
/// machine's own byte order, as Rust holds them.
const STREAM_PLAYBACK: c_int = 0;
const NONBLOCK: c_int = 1;
const FORMAT_S16: c_int = if cfg!(target_endian = "big") { 3 } else { 2 };
const FORMAT_FLOAT: c_int = if cfg!(target_endian = "big") { 15 } else { 14 };
const ACCESS_RW_INTERLEAVED: c_int = 3;
/// How far ahead of what's heard the device's buffer runs, in
/// microseconds: what's played is heard this late at most.
const LATENCY: c_uint = 40_000;

type Handle = *mut c_void;

/// The functions' signatures, as alsa/pcm.h declares them.
type OpenFn = unsafe extern "C" fn(*mut Handle, *const c_char, c_int, c_int) -> c_int;
type SetParamsFn = unsafe extern "C" fn(Handle, c_int, c_int, c_uint, c_uint, c_int, c_uint) -> c_int;
type WriteFn = unsafe extern "C" fn(Handle, *const c_void, c_ulong) -> c_long;
type RecoverFn = unsafe extern "C" fn(Handle, c_int, c_int) -> c_int;
type HandleFn = unsafe extern "C" fn(Handle) -> c_int;
type NonblockFn = unsafe extern "C" fn(Handle, c_int) -> c_int;
type StrErrorFn = unsafe extern "C" fn(c_int) -> *const c_char;
/// alsa-lib's error handler, which C declares with `...` after `fmt`.
type ErrorHandler = unsafe extern "C" fn(*const c_char, c_int, *const c_char, c_int, *const c_char);
type SetErrorHandlerFn = unsafe extern "C" fn(Option<ErrorHandler>) -> c_int;

/// Says nothing. alsa-lib's own handler prints each error to the program's
/// standard error, which for a program built on the SDK is the terminal it
/// draws on, as a machine with no sound device would on every try; the
/// errors come back as numbers anyway. It takes none of the arguments C
/// would pass after `fmt`, which the x86-64 and Arm64 Linux conventions
/// pass as they do fixed ones, so leaving them unread is safe there.
unsafe extern "C" fn say_nothing(_: *const c_char, _: c_int, _: *const c_char, _: c_int, _: *const c_char) {}

/// The functions taken from the library.
struct Alsa {
    open: OpenFn,
    set_params: SetParamsFn,
    writei: WriteFn,
    recover: RecoverFn,
    drop: HandleFn,
    close: HandleFn,
    nonblock: NonblockFn,
    strerror: StrErrorFn,
}

impl Alsa {
    fn error(&self, err: c_int) -> String {
        // SAFETY: snd_strerror gives a static string for any number.
        unsafe { CStr::from_ptr((self.strerror)(err)) }.to_string_lossy().into_owned()
    }
}

/// The library, loaded the first time it's wanted and kept for as long as
/// the program runs; None where it isn't there.
fn alsa() -> Option<&'static Alsa> {
    static ALSA: OnceLock<Option<Alsa>> = OnceLock::new();
    ALSA.get_or_init(load).as_ref()
}

fn load() -> Option<Alsa> {
    // SAFETY: loading the library runs only its own initialisers, and each
    // symbol is the function alsa/pcm.h declares with that signature.
    unsafe {
        let lib = libc::dlopen(c"libasound.so.2".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
        if lib.is_null() {
            return None;
        }
        let sym = |name: &CStr| {
            let f = libc::dlsym(lib, name.as_ptr());
            (!f.is_null()).then_some(f)
        };
        if let Some(set) = sym(c"snd_lib_error_set_handler") {
            std::mem::transmute::<*mut c_void, SetErrorHandlerFn>(set)(Some(say_nothing));
        }
        Some(Alsa {
            open: std::mem::transmute::<*mut c_void, OpenFn>(sym(c"snd_pcm_open")?),
            set_params: std::mem::transmute::<*mut c_void, SetParamsFn>(sym(c"snd_pcm_set_params")?),
            writei: std::mem::transmute::<*mut c_void, WriteFn>(sym(c"snd_pcm_writei")?),
            recover: std::mem::transmute::<*mut c_void, RecoverFn>(sym(c"snd_pcm_recover")?),
            drop: std::mem::transmute::<*mut c_void, HandleFn>(sym(c"snd_pcm_drop")?),
            close: std::mem::transmute::<*mut c_void, HandleFn>(sym(c"snd_pcm_close")?),
            nonblock: std::mem::transmute::<*mut c_void, NonblockFn>(sym(c"snd_pcm_nonblock")?),
            strerror: std::mem::transmute::<*mut c_void, StrErrorFn>(sym(c"snd_strerror")?),
        })
    }
}

/// The default device, open for playing.
pub struct Pcm {
    alsa: &'static Alsa,
    handle: Handle,
    channels: usize,
    /// The samples as 16-bit, where the device wouldn't take floats.
    shorts: Option<Vec<i16>>,
}

// SAFETY: a device's handle may move between threads; only the one holding
// the Pcm uses it.
unsafe impl Send for Pcm {}

impl Pcm {
    /// Opens the default device for `channels` interleaved at `rate`, as
    /// floats, or as 16-bit samples where it won't take those. ALSA
    /// converts the rate and channels to the device's own as it must.
    pub fn open(channels: usize, rate: u32) -> Result<Self, String> {
        let alsa = alsa().ok_or("ALSA's library (libasound.so.2) isn't here")?;
        let mut handle: Handle = std::ptr::null_mut();
        // Opened without waiting, so a device another program holds alone
        // is turned down at once rather than keeping the thread until it's
        // let go. SAFETY: a name and a place for the handle, as
        // snd_pcm_open takes.
        let err = unsafe { (alsa.open)(&mut handle, c"default".as_ptr(), STREAM_PLAYBACK, NONBLOCK) };
        if err < 0 {
            return Err(alsa.error(err));
        }
        // Closed again by its Drop if it won't play.
        let mut pcm = Self { alsa, handle, channels, shorts: None };
        // Writes wait while the buffer is full, which paces the thread.
        // SAFETY: an open handle, and parameters by ALSA's numbers.
        let err = unsafe { (alsa.nonblock)(handle, 0) };
        if err < 0 {
            return Err(alsa.error(err));
        }
        let set = |format| unsafe { (alsa.set_params)(handle, format, ACCESS_RW_INTERLEAVED, channels as c_uint, rate, 1, LATENCY) };
        if set(FORMAT_FLOAT) < 0 {
            let err = set(FORMAT_S16);
            if err < 0 {
                return Err(alsa.error(err));
            }
            pcm.shorts = Some(Vec::new());
        }
        Ok(pcm)
    }

    /// Plays `samples`, interleaved and a whole number of frames, waiting
    /// while the device's buffer is full. A device that ran dry (a busy
    /// machine) starts again; false once it can't play at all.
    pub fn write(&mut self, samples: &[f32]) -> bool {
        let frames = samples.len() / self.channels;
        let (data, size): (*const u8, usize) = match &mut self.shorts {
            Some(shorts) => {
                shorts.clear();
                shorts.extend(samples.iter().map(|s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16));
                (shorts.as_ptr().cast(), 2)
            }
            None => (samples.as_ptr().cast(), 4),
        };
        let mut done = 0;
        while done < frames {
            // SAFETY: the frames not yet written, within the buffer.
            let n = unsafe { (self.alsa.writei)(self.handle, data.add(done * size * self.channels).cast(), (frames - done) as c_ulong) };
            if n >= 0 {
                done += n as usize;
            } else if unsafe { (self.alsa.recover)(self.handle, n as c_int, 1) } < 0 {
                return false;
            }
        }
        true
    }
}

impl Drop for Pcm {
    fn drop(&mut self) {
        // SAFETY: the handle is open, and nothing uses it after this.
        unsafe {
            (self.alsa.drop)(self.handle);
            (self.alsa.close)(self.handle);
        }
    }
}
