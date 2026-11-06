//! Flow's settings from C ([`flow_sdk::settings`], as `flow-settings` uses
//! it): the bytes that ask Flow a JSON request (OSC 7732, its base64), the
//! JSON answer read back out of what came in, and the two together, asked
//! and waited for. The JSON is the caller's to write and to read: the
//! library passes it on as text, so it needs no JSON of its own.
//!
//! On macOS and Linux the SDK asks the program's terminal itself, opened
//! by name, so the question reaches Flow however standard input and
//! output are piped or redirected. The answer comes in as keys, so the
//! terminal has to be raw while it's awaited, or it would sit in the line
//! being typed until Enter, echoed: the SDK makes it raw for the wait where
//! it isn't already (by `flow_raw_mode` or `flow_term_enter`) and puts it
//! back after, so a program reading its input a line at a time can ask
//! between lines, and one that took the terminal keeps it as it was. Where
//! none of standard input, output and error is a terminal it asks nothing.
//! It throws away what waits in the input before it asks, so an answer
//! that came too late for an earlier request isn't taken for this one's.
//! On Windows the question goes down Flow's pipe instead, which needs
//! neither, and Flow's own wait of a few seconds bounds it, not `wait_ms`.

use std::ffi::{c_char, c_void};
use std::time::Duration;

use flow_sdk::settings;

use crate::{owned_or_null, slice, text};

/// How long `flow_settings_ask` waits when given 0: as long as
/// `flow-settings` does.
pub const WAIT_MS: u32 = 3000;

/// The bytes that ask Flow `request` (a JSON object, as text); NULL for
/// NULL or text that isn't UTF-8.
#[no_mangle]
pub unsafe extern "C" fn flow_settings_request(request: *const c_char) -> *mut c_char {
    owned_or_null(|| String::from_utf8(settings::request(text(request)?)).ok())
}

/// The first whole answer in `len` bytes at `bytes`, as JSON text; NULL
/// until one has come whole, or if it isn't one Flow wrote.
#[no_mangle]
pub unsafe extern "C" fn flow_settings_answer(bytes: *const c_void, len: usize) -> *mut c_char {
    owned_or_null(|| settings::answer(slice(bytes.cast::<u8>(), len)?))
}

/// Asks Flow `request` and waits `wait_ms` at most ([`WAIT_MS`] for 0; on
/// Windows, Flow's own few seconds) for its answer, as JSON text; NULL when
/// nothing answered, or where none of standard input, output and error is
/// a terminal, where nothing is asked.
#[no_mangle]
pub unsafe extern "C" fn flow_settings_ask(request: *const c_char, wait_ms: u32) -> *mut c_char {
    owned_or_null(|| {
        let request = text(request)?;
        let wait = Duration::from_millis(u64::from(if wait_ms == 0 { WAIT_MS } else { wait_ms }));
        settings::ask(request, wait).ok().flatten()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flow_sdk::crossterm::terminal;
    use std::ffi::CStr;
    use std::io::IsTerminal;
    use std::time::Instant;

    /// A string the library handed out, read and freed; None for NULL.
    unsafe fn take(s: *mut c_char) -> Option<String> {
        if s.is_null() {
            return None;
        }
        let text = CStr::from_ptr(s).to_str().expect("UTF-8").to_string();
        crate::flow_free(s.cast());
        Some(text)
    }

    #[test]
    fn a_request_goes_as_the_sdks_and_its_answer_is_read_whole() {
        unsafe {
            let asked = take(flow_settings_request(c"{\"do\":\"get\",\"key\":\"fx.style\"}".as_ptr()));
            assert_eq!(asked.as_deref(), Some("\x1b]7732;eyJkbyI6ImdldCIsImtleSI6ImZ4LnN0eWxlIn0=\x07"));
            assert_eq!(take(flow_settings_request(std::ptr::null())), None);
            let reply = b"x\x1b]7732;eyJvayI6dHJ1ZX0=\x1b\\y";
            assert_eq!(take(flow_settings_answer(reply.as_ptr().cast(), reply.len())).as_deref(), Some(r#"{"ok":true}"#));
            assert_eq!(take(flow_settings_answer(reply.as_ptr().cast(), 12)), None, "not come whole yet");
            let other = b"\x1b]11;rgb:0/0/0\x07";
            assert_eq!(take(flow_settings_answer(other.as_ptr().cast(), other.len())), None);
            assert_eq!(take(flow_settings_answer(std::ptr::null(), 3)), None, "NULL with a length");
            assert_eq!(take(flow_settings_answer(std::ptr::null(), 0)), None, "nothing came");
        }
    }

    #[test]
    fn outside_a_terminal_nothing_is_asked_and_nothing_waited_for() {
        // Where any of the three is a terminal, the SDK would ask it.
        let attached = std::io::stdin().is_terminal() || std::io::stdout().is_terminal() || std::io::stderr().is_terminal();
        if (cfg!(unix) && attached) || std::env::var_os(flow_sdk::pipe::VAR).is_some() {
            return;
        }
        let started = Instant::now();
        unsafe {
            assert_eq!(take(flow_settings_ask(c"{\"do\":\"list\"}".as_ptr(), 5000)), None);
            assert_eq!(take(flow_settings_ask(std::ptr::null(), 0)), None);
        }
        assert!(started.elapsed() < Duration::from_secs(1), "came back after {:?}", started.elapsed());
        assert!(!terminal::is_raw_mode_enabled().unwrap_or(false), "raw mode left as it was");
    }
}
