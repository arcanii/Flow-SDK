//! Flow's pipe for images, on Windows. The console layer Windows puts
//! between Flow and a program (ConPTY) passes on only what its console
//! keeps, so it drops kitty's and Ratty's APCs and OSC 7731, and the
//! answers to the questions [`crate::detect`] asks never come. So each Flow
//! tab there opens a named pipe and names it in `FLOW_IMAGE_PIPE`. What a
//! program writes to it is read as if written to the terminal, but only
//! for pictures, models, effects and sounds (named ones and DECPS notes),
//! Flow's settings (OSC 7732, since 0.1.30, [`crate::settings`]), and the
//! questions that go before them (XTVERSION, `CSI 16 t`, `CSI 14 t`,
//! Ratty's `s`, kitty's query, the colors OSC 4, 10, 11 and 12 ask for,
//! since Flow 0.1.28, and DA1, whose answer comes last); the answers come
//! back up the pipe. The cells that show a picture still go through the
//! terminal (`picture` writes them bare there). Elsewhere there's no pipe
//! and the terminal carries everything.

use std::fs::File;
use std::io::{self, Read, Write};
use std::time::Duration;

/// The variable that names a Flow tab's pipe.
pub const VAR: &str = "FLOW_IMAGE_PIPE";

/// DA1, which every terminal answers, and Flow's pipe after the answers to
/// everything before it: reading up to its answer reads them all.
pub const DA1: &[u8] = b"\x1b[c";

/// An open connection to a Flow tab's pipe.
pub struct Pipe {
    file: File,
}

impl Pipe {
    /// Opens the pipe Flow names in `FLOW_IMAGE_PIPE`, on Windows; elsewhere
    /// there's none, whatever the variable says.
    pub fn open() -> Option<Self> {
        if !cfg!(windows) {
            return None;
        }
        let name = std::env::var_os(VAR)?;
        for _ in 0..25 {
            match std::fs::OpenOptions::new().read(true).write(true).open(&name) {
                Ok(file) => return Some(Self { file }),
                // ERROR_PIPE_BUSY: busy for a moment while Flow readies it
                // for the next program.
                Err(e) if e.raw_os_error() == Some(231) => std::thread::sleep(Duration::from_millis(20)),
                Err(_) => return None,
            }
        }
        None
    }

    /// Writes `bytes` down the pipe, answered or not.
    pub fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.file.write_all(bytes)?;
        self.file.flush()
    }

    /// Writes `query`, which must end with [`DA1`], and reads the answers up
    /// to and including DA1's.
    pub fn ask(&mut self, query: &[u8]) -> Vec<u8> {
        self.ask_until(query, answered_da1)
    }

    /// Writes `query` and reads what comes back until `done` says it's all
    /// there, or the pipe closes.
    pub fn ask_until(&mut self, query: &[u8], done: impl Fn(&[u8]) -> bool) -> Vec<u8> {
        let mut got = Vec::new();
        if self.send(query).is_err() {
            return got;
        }
        let mut buf = [0u8; 1024];
        while !done(&got) {
            match self.file.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => got.extend_from_slice(&buf[..n]),
            }
        }
        got
    }

    /// Writes `bytes` and waits until Flow has read them all, by asking DA1
    /// after them: pictures sent this way are in before the cells that show
    /// them, which go the other way, through the terminal.
    pub fn send_and_wait(&mut self, bytes: &[u8]) -> io::Result<()> {
        let got = self.ask(&[bytes, DA1].concat());
        if answered_da1(&got) {
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Flow's pipe closed before it answered"))
        }
    }
}

/// Whether `got` holds DA1's answer, `CSI ? <numbers> c`.
pub fn answered_da1(got: &[u8]) -> bool {
    let text = String::from_utf8_lossy(got);
    text.split("\x1b[?").skip(1).any(|rest| {
        let end = rest.find(|c: char| !(c.is_ascii_digit() || c == ';'));
        end.is_some_and(|i| rest[i..].starts_with('c'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn da1s_answer_is_found_and_nothing_else_passes_for_it() {
        assert!(answered_da1(b"\x1b_Gi=31;OK\x1b\\\x1b[?62;22c"));
        assert!(answered_da1(b"\x1b[?1;2c"));
        assert!(!answered_da1(b"\x1b_Gi=31;OK\x1b\\"));
        assert!(!answered_da1(b"\x1b[?62;22"), "not all of it yet");
        assert!(!answered_da1(b"\x1b[?2026;2$y"), "a mode's report isn't DA1's");
    }

    #[test]
    fn there_is_no_pipe_but_on_windows() {
        if !cfg!(windows) {
            assert!(Pipe::open().is_none());
        }
    }
}
