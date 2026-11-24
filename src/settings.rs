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
//! - `"restart"`: restarts Flow, as its Settings' Restart does, but only
//!   while every other pane (its own tab's others too) waits at its
//!   prompt; otherwise Flow refuses it, `ok` false, and says so in a
//!   toast, since a program asking is never taken for the user's yes, and
//!   restarting then is the user's to do.
//!
//! What fails answers `{"ok": false, "error": "<why, in Flow's language>"}`.
//! Flow doesn't let programs see or change everything: not the assistant's
//! settings, nor the user's own theme, whose prompt the shells expand.
//!
//! From Flow 0.1.46 every request must also carry the asking pane's own
//! secret as its `"pass"`: the [`TAB_KEY`] variable, `FLOW_TAB_KEY`, which
//! Flow gives each pane's shell ([`tab_key`]). Anything printed in a tab
//! could ask otherwise (a file being `cat`ed, a remote host's output over
//! ssh), and Flow's answers are typed into the shell's input. [`ask`] puts
//! the pass in from the variable where the request has none and the
//! variable is set ([`with_pass`]); a request with a pass of its own goes
//! as it is. Without it, or with another pane's, Flow answers nothing at
//! all, not even to refuse. So when [`ask`] gives none, the reason may be
//! any of these: the terminal isn't Flow (or is a terminal started from a
//! Flow tab), or is a Flow from before 0.1.30; there's no terminal to ask;
//! there's no key, or it's another pane's (or one an ssh session or a
//! script brought along); or tmux or screen is in the way, which don't
//! pass the escape on. A program should say why, as `flow-settings` does:
//! with no key (run through sudo or `env -i`, which leave it out, or over
//! ssh, which doesn't carry it) it asks nothing and says so at once, and
//! where nothing answers it names the rest. A Flow from 0.1.30 to 0.1.45
//! answers without a pass, though one from before 0.1.43 gives its tabs no
//! key. [`request`] makes the escape of exactly what it's given, for a
//! program that reads its own input, which puts the pass in itself.
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

use std::borrow::Cow;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::io;
use std::time::Duration;

use base64::Engine as _;

/// The escape's number.
pub const CODE: &str = "7732";

/// The variable that holds the secret of the pane a program runs in, which
/// Flow gives each pane's shell (from 0.1.43) and needs as the `"pass"` of
/// every request (from 0.1.46).
pub const TAB_KEY: &str = "FLOW_TAB_KEY";

/// The bytes that ask Flow `request` (a JSON object, as text), exactly as
/// it's given: a request for Flow 0.1.46 or later carries the pass itself
/// ([`with_pass`]), which [`ask`] adds but this doesn't.
pub fn request(json: &str) -> Vec<u8> {
    format!("\x1b]{CODE};{}\x07", base64::engine::general_purpose::STANDARD.encode(json)).into_bytes()
}

/// The secret of the pane the program runs in ([`TAB_KEY`]): none where
/// the variable isn't set or is empty, as outside Flow's tabs, in a Flow
/// before 0.1.43, or where something on the way left it out (sudo and
/// `env -i` do, and ssh doesn't carry it).
pub fn tab_key() -> Option<String> {
    key_from(std::env::var_os(TAB_KEY))
}

/// The key a variable's `value` gives: none for none, for an empty one,
/// which matches no pane in Flow, and for one that isn't Unicode, which
/// Flow never makes.
fn key_from(value: Option<OsString>) -> Option<String> {
    value?.into_string().ok().filter(|key| !key.is_empty())
}

/// `request` (a JSON object, as text) as [`ask`] sends it: with `key`, the
/// pane's secret, as its `"pass"`, where it has none and `key` is given and
/// isn't empty. Anything else is left as it is: a request with a pass of
/// its own, even an empty or a wrong one, and text that isn't one JSON
/// object, which Flow wouldn't read anyway.
pub fn with_pass<'a>(request: &'a str, key: Option<&str>) -> Cow<'a, str> {
    let Some(key) = key.filter(|key| !key.is_empty()) else { return Cow::Borrowed(request) };
    let Some((names, end)) = members(request) else { return Cow::Borrowed(request) };
    if names.iter().any(|name| name == "pass") {
        return Cow::Borrowed(request);
    }
    let mut sent = String::with_capacity(request.len() + key.len() + 10);
    sent.push_str(&request[..end]);
    if !names.is_empty() {
        sent.push(',');
    }
    sent.push_str("\"pass\":");
    push_string(&mut sent, key);
    sent.push_str(&request[end..]);
    Cow::Owned(sent)
}

/// `text` written as a JSON string, quotes and all, onto `out`.
fn push_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// The names of the members of the JSON object `text` holds, as JSON
/// reads them (escapes and all, so `"pas\u0073"` is `pass`), and where its
/// closing brace is; none for text that isn't one object with nothing but
/// white space around it. Only as much of JSON as finds them: the values
/// are passed over rather than checked, since Flow reads them.
fn members(text: &str) -> Option<(Vec<String>, usize)> {
    let mut scan = Scan { bytes: text.as_bytes(), at: 0 };
    scan.space();
    scan.take(b'{')?;
    let mut names = Vec::new();
    scan.space();
    if scan.peek() != Some(b'}') {
        loop {
            scan.space();
            names.push(scan.string()?);
            scan.space();
            scan.take(b':')?;
            scan.value(0)?;
            scan.space();
            if scan.peek() == Some(b'}') {
                break;
            }
            scan.take(b',')?;
        }
    }
    let end = scan.at;
    scan.take(b'}')?;
    scan.space();
    (scan.at == scan.bytes.len()).then_some((names, end))
}

/// How deep [`Scan::value`] goes into objects and arrays before it gives
/// up: far deeper than any request, and never so deep the stack runs out.
const DEEPEST: usize = 64;

/// A place in JSON text being read.
struct Scan<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Scan<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    /// The next byte, taken.
    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.at += 1;
        Some(byte)
    }

    /// Takes `byte` if it's next; none if something else is.
    fn take(&mut self, byte: u8) -> Option<()> {
        (self.bump()? == byte).then_some(())
    }

    /// Passes over JSON's white space.
    fn space(&mut self) {
        while self.peek().is_some_and(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    /// A string, quotes and all, as what it says. A `\u` escape of half a
    /// surrogate pair reads as U+FFFD, which no name looked for holds.
    fn string(&mut self) -> Option<String> {
        self.take(b'"')?;
        let mut said = Vec::new();
        loop {
            match self.bump()? {
                b'"' => return String::from_utf8(said).ok(),
                b'\\' => {
                    let c = match self.bump()? {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let hex = self.bytes.get(self.at..self.at + 4).filter(|h| h.iter().all(u8::is_ascii_hexdigit))?;
                            self.at += 4;
                            let code = u32::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?;
                            char::from_u32(code).unwrap_or('\u{FFFD}')
                        }
                        _ => return None,
                    };
                    said.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
                }
                // A control character can't be in a string as itself.
                byte if byte < 0x20 => return None,
                byte => said.push(byte),
            }
        }
    }

    /// Passes over a value `depth` objects and arrays down: a string, an
    /// object, an array, or a number, `true`, `false` or `null`, whose
    /// letters aren't checked.
    fn value(&mut self, depth: usize) -> Option<()> {
        if depth > DEEPEST {
            return None;
        }
        self.space();
        match self.peek()? {
            b'"' => self.string().map(drop),
            open @ (b'{' | b'[') => {
                let close = if open == b'{' { b'}' } else { b']' };
                self.at += 1;
                self.space();
                if self.peek() == Some(close) {
                    self.at += 1;
                    return Some(());
                }
                loop {
                    self.space();
                    if open == b'{' {
                        self.string()?;
                        self.space();
                        self.take(b':')?;
                    }
                    self.value(depth + 1)?;
                    self.space();
                    match self.bump()? {
                        b',' => {}
                        byte if byte == close => return Some(()),
                        _ => return None,
                    }
                }
            }
            _ => {
                let start = self.at;
                while self.peek().is_some_and(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'+' | b'.')) {
                    self.at += 1;
                }
                (self.at > start).then_some(())
            }
        }
    }
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
/// seconds instead). The pane's key ([`tab_key`]) goes as the request's
/// `"pass"` where it has none ([`with_pass`]). None if nothing answered:
/// where the terminal isn't Flow, or is a Flow from before 0.1.30; where
/// there's no terminal to ask; where the request has no pass Flow takes
/// (no key, or another pane's), which from 0.1.46 Flow doesn't answer at
/// all; or through tmux or screen, which don't pass the escape on. Keys
/// waiting to be read when it asks, or typed while it waits, are read with
/// the answer and lost.
pub fn ask(request_json: &str, wait: Duration) -> io::Result<Option<String>> {
    let query = request(&with_pass(request_json, tab_key().as_deref()));
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

    #[test]
    fn the_panes_key_goes_as_the_pass_of_a_request_that_has_none() {
        let key = Some("0123456789abcdef0123456789abcdef");
        assert_eq!(with_pass(r#"{"do":"get","key":"fx.style"}"#, key), r#"{"do":"get","key":"fx.style","pass":"0123456789abcdef0123456789abcdef"}"#);
        assert_eq!(with_pass(r#"{"do":"list"}"#, Some("k")), r#"{"do":"list","pass":"k"}"#);
        assert_eq!(with_pass("{}", Some("k")), r#"{"pass":"k"}"#);
        // Written as JSON, in whatever white space and with whatever values.
        assert_eq!(with_pass(" {\n  \"do\": \"set\", \"key\": \"font.family\",\n  \"value\": [\"VT323\", {\"a\": [1, -2.5e3, null]}] }\n", Some("k")), " {\n  \"do\": \"set\", \"key\": \"font.family\",\n  \"value\": [\"VT323\", {\"a\": [1, -2.5e3, null]}] ,\"pass\":\"k\"}\n");
        assert_eq!(with_pass(r#"{"do":"set","key":"x","value":"a \"pass\": \\ }"}"#, Some("k")), r#"{"do":"set","key":"x","value":"a \"pass\": \\ }","pass":"k"}"#, "a pass in a value isn't one");
        assert_eq!(with_pass(r#"{"do":"set","key":"x","value":{"pass":"p"}}"#, Some("k")), r#"{"do":"set","key":"x","value":{"pass":"p"},"pass":"k"}"#, "nor is one inside a value");
        // A key with a quote or a control character in it is still JSON.
        assert_eq!(with_pass("{}", Some("a\"b\\c\u{1}")), r#"{"pass":"a\"b\\c\u0001"}"#);
    }

    #[test]
    fn a_request_with_a_pass_of_its_own_is_sent_as_it_is() {
        for request in [r#"{"do":"list","pass":"mine"}"#, r#"{"pass":"mine","do":"list"}"#, r#"{"do":"list","pass":""}"#, r#"{"do":"list","pass":null}"#, r#"{"do":"list","pas\u0073":"mine"}"#] {
            assert!(matches!(with_pass(request, Some("k")), Cow::Borrowed(r) if r == request), "{request}");
        }
    }

    #[test]
    fn without_a_key_nothing_is_added() {
        let request = r#"{"do":"get","key":"fx.style"}"#;
        assert!(matches!(with_pass(request, None), Cow::Borrowed(r) if r == request));
        assert!(matches!(with_pass(request, Some("")), Cow::Borrowed(r) if r == request), "an empty key matches no pane");
        // The variable as it's read: none, empty or not Unicode is no key.
        assert_eq!(key_from(None), None);
        assert_eq!(key_from(Some(OsString::new())), None);
        assert_eq!(key_from(Some(OsString::from("k1"))), Some("k1".into()));
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            assert_eq!(key_from(Some(OsString::from_vec(vec![0xff, b'k']))), None);
        }
    }

    #[test]
    fn what_isnt_one_json_object_is_left_as_it_is() {
        let deep = format!("{{\"do\":\"get\",\"value\":{}1{}}}", "[".repeat(DEEPEST + 2), "]".repeat(DEEPEST + 2));
        for request in ["", "[1]", "\"list\"", "{", "{\"do\":\"list\"", "{\"do\":\"list\"}{}", "{\"do\":\"list\"} x", "{\"do\" \"list\"}", "{\"do\":\"list\",}", "{do:\"list\"}", "{\"do\":}", "{\"do\":\"li\nst\"}", "{\"do\":\"\\x\"}", "{\"do\":\"\\u12\"}", deep.as_str()] {
            assert!(matches!(with_pass(request, Some("k")), Cow::Borrowed(r) if r == request), "{request:?}");
        }
        // As deep as a request might go is still read.
        let nested = format!("{{\"value\":{}1{}}}", "[".repeat(DEEPEST), "]".repeat(DEEPEST));
        assert!(with_pass(&nested, Some("k")).ends_with(",\"pass\":\"k\"}"));
    }
}
