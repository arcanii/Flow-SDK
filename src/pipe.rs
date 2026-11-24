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
//!
//! A pipe's name is anyone's to take: once a tab closes, or its pipe stops
//! being served, a program from it that's still running could open
//! whatever took the name next. So [`Pipe::open`] opens it at the level of
//! identification, so whatever serves it can learn who opened it but never
//! act as them, and writes nothing to a pipe whose owner isn't the user
//! the program runs as, which every pipe Flow makes is (from 0.1.46 it
//! names the user as its owner, and so did the system before for a Flow
//! that wasn't run as administrator), and none another user makes can be.

use std::ffi::OsStr;
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
    /// Opens the pipe Flow names in `FLOW_IMAGE_PIPE`, on Windows, where
    /// it's owned by the user the program runs as; elsewhere there's none,
    /// whatever the variable says. One another user owns is closed again
    /// unwritten, as if there were none.
    pub fn open() -> Option<Self> {
        if !cfg!(windows) {
            return None;
        }
        let name = std::env::var_os(VAR)?;
        for _ in 0..25 {
            match open_as_client(&name) {
                Ok(file) => return owned_by_user(&file).then_some(Self { file }),
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

/// Opens the pipe `name` to read and write, at the level of identification
/// (`SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION` among CreateFileW's
/// flags, which std's `security_qos_flags` puts there), as Flow's own
/// `flow-mcp` opens its pipe. Asked for no level, as std's `OpenOptions`
/// asks by default, a pipe's client lets whatever serves it act as them
/// once it has read what they wrote, before the server is known to be
/// Flow: a program that took the name under an account that may
/// impersonate (a service's) could start programs as the user.
#[cfg(windows)]
fn open_as_client(name: &OsStr) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT};
    std::fs::OpenOptions::new().read(true).write(true).security_qos_flags(SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION).open(name)
}

#[cfg(not(windows))]
fn open_as_client(_name: &OsStr) -> io::Result<File> {
    Err(io::ErrorKind::Unsupported.into())
}

/// Whether the pipe `file` is an end of is owned by the user this program
/// runs as, by what the system says of both.
#[cfg(windows)]
fn owned_by_user(file: &File) -> bool {
    use std::os::windows::io::AsRawHandle;
    owner_is_user(sid::owner(file.as_raw_handle()).as_deref(), sid::user().as_deref())
}

#[cfg(not(windows))]
fn owned_by_user(_file: &File) -> bool {
    // There's no pipe here, so nothing is known of who owns one.
    owner_is_user(None, None)
}

/// Whether a pipe may be written to, by its owner's SID and the SID of the
/// user the program runs as, each as its bytes, none where it couldn't be
/// read: only where both were read and are the same user. Only a user can
/// make themselves an object's owner (anyone else would need the system's
/// own rights), so a pipe another user made never passes, whoever serves
/// it and whatever it says. Two whole SIDs' bytes are the same just where
/// the system's `EqualSid` says they are.
fn owner_is_user(owner: Option<&[u8]>, user: Option<&[u8]>) -> bool {
    matches!((owner, user), (Some(owner), Some(user)) if is_sid(user) && owner == user)
}

/// Whether `bytes` are a whole SID as Windows lays one out: revision 1,
/// the count of its sub-authorities (at most 15), six bytes of identifier
/// authority, and the sub-authorities, four bytes each.
fn is_sid(bytes: &[u8]) -> bool {
    matches!(bytes, [1, count, ..] if *count <= 15 && bytes.len() == 8 + 4 * usize::from(*count))
}

/// The SIDs [`owner_is_user`] compares, read from the system as bytes, as
/// Flow's own `owned_by` reads them (`windows/src/pipe.rs`).
#[cfg(windows)]
mod sid {
    use std::os::windows::io::RawHandle;

    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, ERROR_SUCCESS, HANDLE};
    use windows_sys::Win32::Security::Authorization::{GetSecurityInfo, SE_KERNEL_OBJECT};
    use windows_sys::Win32::Security::{GetLengthSid, GetTokenInformation, IsValidSid, TokenUser, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, TOKEN_QUERY, TOKEN_USER};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    /// The bytes of the SID at `sid`, where it's a valid one.
    ///
    /// # Safety
    ///
    /// `sid` is null or points at memory that stays alive while it's read.
    unsafe fn bytes(sid: PSID) -> Option<Vec<u8>> {
        // SAFETY: a SID the caller keeps alive, checked to be one before
        // its length is read, then read no further than that.
        unsafe {
            if sid.is_null() || IsValidSid(sid) == 0 {
                return None;
            }
            let len = GetLengthSid(sid) as usize;
            Some(std::slice::from_raw_parts(sid.cast::<u8>(), len).to_vec())
        }
    }

    /// The owner of what `handle` is open to, which needs READ_CONTROL, as
    /// opening it to read gives.
    pub(super) fn owner(handle: RawHandle) -> Option<Vec<u8>> {
        let mut owner: PSID = std::ptr::null_mut();
        let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: an open handle; `owner` points into the descriptor made,
        // which is read before it's freed below.
        let got = unsafe { GetSecurityInfo(handle, SE_KERNEL_OBJECT, OWNER_SECURITY_INFORMATION, &mut owner, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), &mut descriptor) };
        // SAFETY: the owner it gave, in the descriptor, alive till below.
        let sid = if got == ERROR_SUCCESS { unsafe { bytes(owner) } } else { None };
        if !descriptor.is_null() {
            // SAFETY: memory GetSecurityInfo gave, no longer read.
            unsafe { LocalFree(descriptor) };
        }
        sid
    }

    /// The user this process runs as, as its token names them.
    pub(super) fn user() -> Option<Vec<u8>> {
        let mut token: HANDLE = std::ptr::null_mut();
        // SAFETY: this process's pseudo-handle, which needs no closing; its
        // token, opened to be read, is closed below.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return None;
        }
        let mut size = 0u32;
        // SAFETY: asks the size first, then fills a buffer of words (so the
        // TOKEN_USER at its start is aligned) at least that big.
        unsafe { GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut size) };
        let mut buf = vec![0u64; (size as usize).div_ceil(8)];
        let got = unsafe { GetTokenInformation(token, TokenUser, buf.as_mut_ptr().cast(), size, &mut size) };
        // SAFETY: the token opened above.
        unsafe { CloseHandle(token) };
        if got == 0 || (size as usize) < std::mem::size_of::<TOKEN_USER>() {
            return None;
        }
        // SAFETY: a TOKEN_USER fills the buffer's start, and its SID points
        // into the buffer, alive here.
        unsafe { bytes((*buf.as_ptr().cast::<TOKEN_USER>()).User.Sid) }
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

    /// The SID written `S-1-<authority>-<sub-authorities>…`, as Windows lays
    /// it out: its authority big-endian, its sub-authorities little-endian.
    fn sid(text: &str) -> Vec<u8> {
        let parts: Vec<u64> = text.strip_prefix("S-").unwrap().split('-').map(|p| p.parse().unwrap()).collect();
        let mut bytes = vec![parts[0] as u8, (parts.len() - 2) as u8];
        bytes.extend_from_slice(&parts[1].to_be_bytes()[2..]);
        for part in &parts[2..] {
            bytes.extend_from_slice(&(*part as u32).to_le_bytes());
        }
        bytes
    }

    #[test]
    fn a_pipe_is_written_to_only_where_its_owner_is_the_user_the_program_runs_as() {
        let me = sid("S-1-5-21-3623811015-3361044348-30300820-1001");
        assert_eq!(me.len(), 28);
        assert!(owner_is_user(Some(&me), Some(&me)));
        for (owner, who) in [
            ("S-1-5-21-3623811015-3361044348-30300820-1002", "another user of this computer"),
            ("S-1-5-21-1004336348-1177238915-682003330-1001", "another computer's or domain's user of the same number"),
            ("S-1-5-32-544", "Administrators, an elevated program's owner by default"),
            ("S-1-5-18", "the system's own account"),
            ("S-1-5-21-3623811015-3361044348-30300820", "the user's domain without the user"),
        ] {
            assert!(!owner_is_user(Some(&sid(owner)), Some(&me)), "{who}");
        }
        // Where either couldn't be read, nothing is written.
        assert!(!owner_is_user(None, Some(&me)), "an owner unknown");
        assert!(!owner_is_user(Some(&me), None), "the user unknown");
        assert!(!owner_is_user(None, None));
        // Nor where what was read isn't a whole SID, however alike.
        for bad in [&[][..], &me[..27], &[2, 0, 0, 0, 0, 0, 0, 5][..], &[1, 16, 0, 0, 0, 0, 0, 5][..]] {
            assert!(!owner_is_user(Some(bad), Some(bad)), "{bad:?}");
        }
        assert!(is_sid(&sid("S-1-5-18")) && is_sid(&sid("S-1-1-0")));
    }
}
