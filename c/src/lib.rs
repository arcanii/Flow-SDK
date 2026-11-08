//! Flow's SDK for C, and for any language that calls C (Python's ctypes,
//! say): the questions, pictures, models, meshes, scenes and their vector
//! maths, stages, screens of cells, effects, Flow's settings and its theme
//! packages of `flow-sdk`, and with the `audio` feature (on by default)
//! its songs and mixer, as `extern "C"` functions named `flow_…`, declared
//! by hand in `include/flow_sdk.h`. The SDK's functions mostly make escape
//! strings, and so do these: each comes back as a string or a buffer that
//! the caller owns and hands back to [`flow_free`]. The builders are
//! opaque handles (`FlowMesh`, `FlowEffect`, `FlowScene`, `FlowCamera`,
//! `FlowStage`, `FlowScreen`, `FlowTune`, `FlowSong`, `FlowAudio`,
//! `FlowMixer`, `FlowTheme`, `FlowThemePackage`, and `FlowTerminal` for
//! what the terminal said), each freed by its own `_free`.
//!
//! What every function keeps to, which the header says again for C:
//!
//! - A NULL pointer is never read: where one isn't allowed the function
//!   does nothing and says so (NULL, -1 or false back).
//! - A pointer with a length is read for that length only, during the call,
//!   and never kept; a NULL one with a length of 0 is an empty slice.
//! - A returned string is NUL-terminated UTF-8 and the caller's until it
//!   goes back to [`flow_free`]; a returned buffer also says its length.
//! - Nothing unwinds across the boundary: each body runs under
//!   `catch_unwind`, and a panic comes back as NULL or -1.
//!
//! The functions marked `unsafe` read raw pointers, and the header is where
//! a C programmer reads what each must point at, so they don't repeat it
//! as `# Safety` sections here.
#![allow(clippy::missing_safety_doc)]

pub mod audio;
pub mod fx;
pub mod mesh;
pub mod model;
pub mod picture;
pub mod scene;
pub mod screen;
pub mod settings;
pub mod stage;
pub mod term;
pub mod theme;

use std::alloc::{alloc, dealloc, Layout};
use std::ffi::{c_char, c_int, c_void, CStr};
use std::io;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// The version of this interface, `FLOW_SDK_ABI` in the header. It goes up
/// whenever a function or a struct changes in a way an older caller would
/// misread, so a wrapper can refuse a library it wasn't written for.
pub const ABI: u32 = 1;

/// Bytes kept in front of each string or buffer handed out, holding the
/// allocation's size so [`flow_free`] needs only the pointer; 16 keeps what
/// follows aligned as malloc's would be.
const HEADER: usize = 16;

#[no_mangle]
pub extern "C" fn flow_abi_version() -> u32 {
    ABI
}

/// Frees a string or a buffer that a `flow_` function handed out. NULL is
/// let be. Handles have their own `_free`s.
#[no_mangle]
pub unsafe extern "C" fn flow_free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let base = ptr.cast::<u8>().sub(HEADER);
    let size = base.cast::<usize>().read();
    dealloc(base, Layout::from_size_align_unchecked(size, HEADER));
}

/// Runs `body`, giving `fallback` if it panics: nothing unwinds into C.
pub(crate) fn guard<T>(fallback: T, body: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or(fallback)
}

/// `bytes` copied into memory the caller frees with [`flow_free`], with a
/// NUL after them so text reads as a C string; NULL if there's no memory.
pub(crate) fn hand_over(bytes: &[u8]) -> *mut u8 {
    let Some(size) = bytes.len().checked_add(HEADER + 1) else { return std::ptr::null_mut() };
    let Ok(layout) = Layout::from_size_align(size, HEADER) else { return std::ptr::null_mut() };
    // SAFETY: the layout's size is never 0, and every byte written lies
    // inside it: the size in the header, then the bytes and the NUL.
    unsafe {
        let base = alloc(layout);
        if base.is_null() {
            return std::ptr::null_mut();
        }
        base.cast::<usize>().write(size);
        let data = base.add(HEADER);
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
        data.add(bytes.len()).write(0);
        data
    }
}

/// A string handed out as a C string.
pub(crate) fn owned(text: String) -> *mut c_char {
    hand_over(text.as_bytes()).cast()
}

/// Clears `*error`, if `error` points anywhere, so the caller always has
/// NULL or a reason to free.
pub(crate) unsafe fn untold(error: *mut *mut c_char) {
    if let Some(error) = error.as_mut() {
        *error = std::ptr::null_mut();
    }
}

/// Puts `why` in `*error`, if `error` points anywhere, for the caller to
/// free.
pub(crate) unsafe fn tell(error: *mut *mut c_char, why: &str) {
    if let Some(error) = error.as_mut() {
        *error = owned(why.to_string());
    }
}

/// A string made under [`guard`], or NULL if it couldn't be made or
/// panicked.
pub(crate) fn owned_or_null(body: impl FnOnce() -> Option<String>) -> *mut c_char {
    guard(std::ptr::null_mut(), || body().map_or(std::ptr::null_mut(), owned))
}

/// `len` items at `ptr`: none for a length of 0, whatever the pointer; None
/// for NULL with a length.
pub(crate) unsafe fn slice<'a, T>(ptr: *const T, len: usize) -> Option<&'a [T]> {
    if len == 0 {
        Some(&[])
    } else if ptr.is_null() {
        None
    } else {
        Some(std::slice::from_raw_parts(ptr, len))
    }
}

/// A NUL-terminated UTF-8 string; None for NULL or text that isn't UTF-8.
pub(crate) unsafe fn text<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok()
}

/// 0 for done, -1 for failed: what C expects back.
pub(crate) fn status(result: io::Result<()>) -> c_int {
    if result.is_ok() {
        0
    } else {
        -1
    }
}

/// A float the caller left unset: NAN (or any value that isn't finite).
pub(crate) fn given(value: f32) -> Option<f32> {
    value.is_finite().then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_handed_over_reads_as_a_c_string_and_frees() {
        let s = owned("\x1b[c".to_string());
        // SAFETY: just handed out, NUL-terminated, freed once.
        unsafe {
            assert_eq!(CStr::from_ptr(s).to_bytes(), b"\x1b[c");
            flow_free(s.cast());
            flow_free(std::ptr::null_mut());
        }
        let empty = hand_over(&[]);
        assert!(!empty.is_null(), "an empty buffer is still one to free");
        unsafe { flow_free(empty.cast()) };
    }

    #[test]
    fn a_panic_comes_back_as_the_fallback() {
        let prior = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        assert_eq!(guard(-1, || panic!("not across the boundary")), -1);
        std::panic::set_hook(prior);
        assert!(owned_or_null(|| None).is_null());
    }

    #[test]
    fn a_null_pointer_is_an_empty_slice_only_with_no_length() {
        unsafe {
            assert_eq!(slice::<u8>(std::ptr::null(), 0), Some(&[][..]));
            assert_eq!(slice::<u8>(std::ptr::null(), 3), None);
            assert_eq!(text(std::ptr::null()), None);
            assert_eq!(text(c"chime".as_ptr()), Some("chime"));
        }
        assert_eq!((given(f32::NAN), given(f32::INFINITY), given(2.5)), (None, None, Some(2.5)));
    }
}
