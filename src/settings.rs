//! Flow's settings, asked about and changed by a program in one of its tabs
//! (as `flow-settings` does), with OSC 7732: `ESC ] 7732 ; <base64> BEL`,
//! the base64 of a JSON request, answered the same way with the base64 of
//! a JSON answer.
//!
//! A request is `{"do": …}` with, as it needs them, `"key"` (a dotted name
//! from Flow's config, as `fx.style`) and `"value"` (any JSON):
//!
//! - `"list"`: every setting a program may see, as `{"ok": true,
//!   "settings": {"fx.style": "rain", …}}`;
//! - `"get"`: one, as `{"ok": true, "key": …, "value": …}`;
//! - `"set"`: changes one, answered with what it is now (Flow applies it,
//!   saves it, and says so in a toast);
//! - `"reset"`: puts one back to its default, or with `"key": "all"` every
//!   one a program may change;
//! - `"reset-terminal"`: starts the asking tab afresh;
//! - `"restart"`: restarts Flow, as its Settings' Restart does, at once
//!   while every other tab waits at its prompt, else asking first.
//!
//! What fails answers `{"ok": false, "error": "<why, in Flow's language>"}`.
//! Flow doesn't let programs see or change everything: not the assistant's
//! settings, nor the user's own theme, whose prompt the shells expand.
//!
//! On Windows the request goes down Flow's pipe ([`crate::pipe`]), since the
//! console layer between Flow and the program drops it, and so does the
//! answer. The pipe can't be read against a clock, so the wait there is
//! Flow's own, three seconds or so, after which only the DA1 that [`ask`]
//! puts after the request is answered. Elsewhere it goes to the program's
//! terminal itself, opened afresh by name (`ttyname_r` of whichever of
//! standard input, error and output is it; macOS's `poll` can't wait on
//! `/dev/tty`), not to standard input and output, so it
//! reaches Flow and comes back however those are piped, redirected or read
//! by the shell's `$(…)`; the answer comes in as keys, so [`ask`] takes raw
//! mode on that terminal for its wait where it isn't raw already (no line
//! editing, no echo) and puts it back after, and nothing else should be
//! reading the keys meanwhile. Where none of standard input, output and
//! error is a terminal (a program run with all three taken, as tests run
//! one), it asks nothing. An answer that comes after its wait has run out
//! lands in the terminal's input: [`ask`] throws away what waits there
//! before it asks, so the next request doesn't take it for its own, but
//! meanwhile a terminal that isn't raw echoes it, and a program that reads
//! lines reads it.

use std::io;
use std::time::Duration;

use base64::Engine as _;

/// The escape's number.
pub const CODE: &str = "7732";

/// The bytes that ask Flow `request` (a JSON object, as text).
pub fn request(json: &str) -> Vec<u8> {
    format!("\x1b]{CODE};{}\x07", base64::engine::general_purpose::STANDARD.encode(json)).into_bytes()
}

/// The first whole answer in `bytes`, as JSON text; none until one has
/// come whole, or if it isn't one Flow wrote.
pub fn answer(bytes: &[u8]) -> Option<String> {
    let start = bytes.windows(7).position(|w| w == b"\x1b]7732;")? + 7;
    let rest = &bytes[start..];
    let end = rest.iter().position(|&b| b == 0x07 || b == 0x1b)?;
    let text = base64::engine::general_purpose::STANDARD.decode(&rest[..end]).ok()?;
    String::from_utf8(text).ok()
}

/// Asks Flow `request` (a JSON object, as text) and waits for its answer
/// (a JSON object, as text), `wait` at most (on Windows, Flow's own few
/// seconds instead): none if nothing answered, as where the terminal isn't
/// Flow, or is a Flow from before 0.1.30, or where there's no terminal to
/// ask. Keys waiting to be read when it asks, or typed while it waits, are
/// read with the answer and lost.
pub fn ask(request_json: &str, wait: Duration) -> io::Result<Option<String>> {
    let query = request(request_json);
    let got = if cfg!(windows) {
        // DA1 after it, which Flow's pipe answers after everything before
        // it, so a Flow that couldn't answer in time, or one from before
        // 0.1.30, still ends the read, which nothing else would.
        match crate::pipe::Pipe::open() {
            Some(mut pipe) => pipe.ask(&[&query[..], crate::pipe::DA1].concat()),
            None => return Ok(None),
        }
    } else {
        ask_terminal(&query, wait)
    };
    Ok(answer(&got))
}

/// Asks the program's terminal itself, raw for the wait.
#[cfg(unix)]
fn ask_terminal(query: &[u8], wait: Duration) -> Vec<u8> {
    let Some(tty) = Tty::open() else { return Vec::new() };
    tty.throw_away_input();
    if !tty.write_all(query) {
        return Vec::new();
    }
    crate::detect::read_answers(tty.fd, wait, |got| answer(got).is_some())
}

#[cfg(not(unix))]
fn ask_terminal(_query: &[u8], _wait: Duration) -> Vec<u8> {
    Vec::new()
}

/// The program's terminal, opened afresh by name and raw for as long as
/// it's kept: put back as it was, and closed, when dropped.
#[cfg(unix)]
struct Tty {
    fd: libc::c_int,
    /// How it was before it was made raw, if this made it so.
    was: Option<libc::termios>,
}

#[cfg(unix)]
impl Tty {
    /// The terminal that standard input, error or output is, the first of
    /// them that is one: none with all three taken (by a test, or a script
    /// run from no terminal), which asks nothing.
    fn open() -> Option<Self> {
        // SAFETY: isatty takes no pointers.
        let of = [libc::STDIN_FILENO, libc::STDERR_FILENO, libc::STDOUT_FILENO].into_iter().find(|&fd| unsafe { libc::isatty(fd) } == 1)?;
        let mut name = [0 as libc::c_char; 256];
        // SAFETY: ttyname_r writes at most the buffer's length, ended by a
        // nul when it succeeds; open reads that name.
        if unsafe { libc::ttyname_r(of, name.as_mut_ptr(), name.len()) } != 0 {
            return None;
        }
        let fd = unsafe { libc::open(name.as_ptr(), libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC) };
        if fd < 0 {
            return None;
        }
        let mut tty = Tty { fd, was: None };
        // SAFETY: termios is plain data, filled in by tcgetattr.
        let mut now: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut now) } != 0 {
            return None;
        }
        // Raw enough for an answer: no line to wait for Enter on, no echo.
        // A program that made it raw itself (crossterm's raw mode) is left
        // as it is.
        if now.c_lflag & (libc::ICANON | libc::ECHO) != 0 {
            let mut raw = now;
            raw.c_lflag &= !(libc::ICANON | libc::ECHO);
            raw.c_cc[libc::VMIN] = 1;
            raw.c_cc[libc::VTIME] = 0;
            // SAFETY: a termios from tcgetattr, changed in its flags.
            if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) } != 0 {
                return None;
            }
            tty.was = Some(now);
        }
        Some(tty)
    }

    /// Throws away the terminal's input that's waiting to be read: an
    /// answer that came after an earlier request's wait ran out would be
    /// taken for this one's. Keys typed ahead go too, as they would with
    /// the answer.
    fn throw_away_input(&self) {
        // SAFETY: no pointers.
        unsafe { libc::tcflush(self.fd, libc::TCIFLUSH) };
    }

    /// Writes all of `bytes`; false if the terminal wouldn't take them.
    fn write_all(&self, mut bytes: &[u8]) -> bool {
        while !bytes.is_empty() {
            // SAFETY: a buffer that outlives the call, its length given.
            let n = unsafe { libc::write(self.fd, bytes.as_ptr().cast(), bytes.len()) };
            if n < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if n <= 0 {
                return false;
            }
            bytes = &bytes[n as usize..];
        }
        true
    }
}

#[cfg(unix)]
impl Drop for Tty {
    fn drop(&mut self) {
        // SAFETY: the termios tcgetattr gave, and the descriptor open made.
        unsafe {
            if let Some(was) = self.was {
                libc::tcsetattr(self.fd, libc::TCSANOW, &was);
            }
            libc::close(self.fd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_and_its_answer_go_as_base64_of_json() {
        let asked = request(r#"{"do":"get","key":"fx.style"}"#);
        assert_eq!(asked, b"\x1b]7732;eyJkbyI6ImdldCIsImtleSI6ImZ4LnN0eWxlIn0=\x07");
        // An answer among other input, ended by BEL or ST, read whole only.
        let reply = b"x\x1b]7732;eyJvayI6dHJ1ZX0=\x1b\\y";
        assert_eq!(answer(reply).as_deref(), Some(r#"{"ok":true}"#));
        assert_eq!(answer(&reply[..12]), None, "not come whole yet");
        assert_eq!(answer(b"\x1b]11;rgb:0/0/0\x07"), None);
    }
}
