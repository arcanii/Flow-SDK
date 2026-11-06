//! A demo's way out when its terminal goes. Closing a tab hangs its
//! terminal up, and the hang-up's signal (SIGHUP, which the shell passes on
//! to its jobs too) ends a process on the spot by default, its destructors
//! unrun: so the balloon's file stayed behind. Here SIGHUP and SIGTERM set a
//! flag instead ([`caught`]), which the demos' loops look at each frame,
//! ending as on q, and the balloon's file goes as its `ModelFile` is
//! dropped. SIGINT is left be: in raw mode Ctrl+C comes as a key, which
//! quits already.
//!
//! A loop waiting for keys may never get to look, though: crossterm reads a
//! hung-up terminal's end of input over and over without giving up, and a
//! terminal that has stopped reading holds the loop's write (and std's lock
//! on stdout with it) for as long as it doesn't read. So the handler also
//! wakes a thread of the demo's own, which gives the loop [`GRACE`] to end
//! and then, as it hasn't, deletes the files noted with [`delete_at_close`]
//! itself, hands the terminal back if it can within a moment, and ends the
//! process by the signal it was sent, as it would have ended without the
//! handler. On Windows, closing the console (`CTRL_CLOSE_EVENT`) gives the
//! process a few seconds and then ends it wherever it is, so there the
//! handler, which runs on a thread of its own, deletes them at once.
//!
//! What a run killed outright left (SIGKILL, a crash) the next one sweeps
//! (`balloon::sweep`), asking here whether the process a folder is named
//! for still runs ([`running`]) and whether the folder is this user's alone
//! ([`mine`]). It's all the C library's `signal`, `raise`, `write`, `kill`
//! and `getuid`, and kernel32's `SetConsoleCtrlHandler`, `OpenProcess` and
//! `GetExitCodeProcess`, which std links already, as `clock.rs` has it: no
//! crate.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// How long the demo's own loop has, once told to end, before what it made
/// is deleted for it and the process ended: a few of its frames.
pub const GRACE: Duration = Duration::from_millis(500);

/// Set once the terminal has hung up or the demo was told to end.
static CAUGHT: AtomicBool = AtomicBool::new(false);

/// What's deleted if the demo's own thread can't get to it.
static LEFT: Left = Left::new();

/// Whether the terminal has hung up, or the demo was told to end, since
/// [`catch`].
pub fn caught() -> bool {
    CAUGHT.load(Ordering::SeqCst)
}

/// Notes `file`, in its folder `dir`, to be deleted with it if the demo
/// ends where its own thread can't get to them.
pub fn delete_at_close(file: &Path, dir: &Path) {
    LEFT.keep(file, dir);
}

/// Takes the folder `dir` off that list, once it's been deleted.
pub fn deleted(dir: &Path) {
    LEFT.forget(dir);
}

/// Files, each with its folder, to delete from another thread than the
/// demo's own.
struct Left(Mutex<Vec<(PathBuf, PathBuf)>>);

impl Left {
    const fn new() -> Self {
        Self(Mutex::new(Vec::new()))
    }

    /// The list, even after a panic elsewhere: it's only paths.
    fn list(&self) -> MutexGuard<'_, Vec<(PathBuf, PathBuf)>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn keep(&self, file: &Path, dir: &Path) {
        self.list().push((file.to_path_buf(), dir.to_path_buf()));
    }

    fn forget(&self, dir: &Path) {
        self.list().retain(|(_, d)| d != dir);
    }

    /// Deletes each file, then its folder if nothing else is in it.
    fn delete_all(&self) {
        for (file, dir) in self.list().drain(..) {
            let _ = fs::remove_file(file);
            let _ = fs::remove_dir(dir);
        }
    }
}

#[cfg(unix)]
mod unix {
    use std::ffi::{c_int, c_void};
    use std::io::{self, Read};
    use std::os::fd::IntoRawFd;
    use std::os::unix::net::UnixStream;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::mpsc;
    use std::time::Duration;

    use super::{Left, CAUGHT, GRACE, LEFT};

    // The same numbers on macOS and Linux.
    pub const SIGHUP: c_int = 1;
    pub const SIGTERM: c_int = 15;

    /// How long the watcher waits for the terminal to be handed back before
    /// ending the process without: a terminal that reads takes it at once.
    const HAND_BACK: Duration = Duration::from_millis(250);

    extern "C" {
        // `None` is `SIG_DFL`, the default action. Returns the handler
        // before (`sighandler_t`), or `SIG_ERR`.
        fn signal(signum: c_int, handler: Option<extern "C" fn(c_int)>) -> usize;
        fn raise(sig: c_int) -> c_int;
        fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
    }

    /// Which signal came, for the process to end by.
    pub static SIGNAL: AtomicI32 = AtomicI32::new(0);
    /// Where the handler wakes the watcher: its socket's other end, kept
    /// open for good, or -1 without one.
    static WAKE: AtomicI32 = AtomicI32::new(-1);

    /// All a signal handler does here: notes the signal and, the first
    /// time, wakes the watcher. Atomics and `write` are safe in a handler,
    /// and a byte written into an empty socket goes, leaving `errno` as the
    /// interrupted code had it.
    pub extern "C" fn on_signal(signum: c_int) {
        SIGNAL.store(signum, Ordering::SeqCst);
        if !CAUGHT.swap(true, Ordering::SeqCst) {
            let fd = WAKE.load(Ordering::SeqCst);
            if fd >= 0 {
                // SAFETY: one byte from a live buffer, to a socket never closed.
                unsafe { write(fd, [1u8].as_ptr().cast(), 1) };
            }
        }
    }

    pub fn catch() {
        // The watcher first, so a signal that comes at once finds it.
        if let Ok((wake, woken)) = UnixStream::pair() {
            let end = || {
                // On a thread of its own: a terminal that has stopped
                // reading holds std's lock on stdout with the loop's write,
                // and would hold this thread too. One that reads gets its
                // main screen and modes back; one that doesn't has the shell
                // put its modes back, as for any job a signal ends; a
                // hung-up one has no one to tell.
                within(HAND_BACK, flow_sdk::term::leave);
                end_by(SIGNAL.load(Ordering::SeqCst));
            };
            let spawned = std::thread::Builder::new().name("flow-demo-hangup".into()).spawn(move || watch(woken, GRACE, &LEFT, end));
            if spawned.is_ok() {
                WAKE.store(wake.into_raw_fd(), Ordering::SeqCst);
            }
        }
        take_signals();
    }

    pub fn take_signals() {
        for signum in [SIGHUP, SIGTERM] {
            // SAFETY: a handler that does only what's safe in one, for as
            // long as the process runs. macOS's and glibc's `signal` keep it
            // in place and go on with the calls it interrupts, as BSD's did.
            unsafe { signal(signum, Some(on_signal)) };
        }
    }

    /// The watcher: once woken, gives the demo's own loop `grace` to end
    /// and, as it hasn't, deletes what's `left` and `end`s it. A socket
    /// closed without a word wakes it to nothing.
    pub fn watch(mut woken: impl Read, grace: Duration, left: &Left, end: impl FnOnce()) {
        let mut byte = [0];
        loop {
            match woken.read(&mut byte) {
                Ok(n) if n > 0 => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                _ => return,
            }
        }
        std::thread::sleep(grace);
        left.delete_all();
        end();
    }

    /// Runs `f` on a thread of its own and waits at most `wait` for it:
    /// whether it was done in time. One still at it is left behind.
    pub fn within(wait: Duration, f: impl FnOnce() + Send + 'static) -> bool {
        let (done, finished) = mpsc::channel();
        let spawned = std::thread::Builder::new().name("flow-demo-leave".into()).spawn(move || {
            f();
            let _ = done.send(());
        });
        spawned.is_ok() && finished.recv_timeout(wait).is_ok()
    }

    /// Ends the process by `signum`, its default action back in place, so
    /// whoever started the demo hears which signal ended it, as before
    /// there was a handler.
    fn end_by(signum: c_int) -> ! {
        // SAFETY: the default action back, then the signal to this thread,
        // which ends the whole process.
        unsafe {
            signal(signum, None);
            raise(signum);
        }
        // Only were the signal not to end it.
        std::process::exit(128 + signum)
    }

    /// Whether process `pid` runs, as far as the system says: one it won't
    /// say of (another user's) runs, so what it left stays.
    pub fn running(pid: u32) -> bool {
        // "No such process", the same on macOS and Linux.
        const ESRCH: i32 = 3;
        extern "C" {
            fn kill(pid: c_int, sig: c_int) -> c_int;
        }
        // Nought and below name groups of processes, never one.
        let Some(pid) = c_int::try_from(pid).ok().filter(|&p| p > 0) else { return true };
        // SAFETY: signal 0 sends nothing; it only asks whether the process is there.
        let there = unsafe { kill(pid, 0) } == 0;
        there || io::Error::last_os_error().raw_os_error() != Some(ESRCH)
    }

    /// Whether a folder is this user's and open to no one else, as the
    /// demo makes its own.
    pub fn mine(meta: &std::fs::Metadata) -> bool {
        use std::os::unix::fs::MetadataExt;
        extern "C" {
            fn getuid() -> u32;
        }
        // SAFETY: getuid only answers.
        meta.uid() == unsafe { getuid() } && meta.mode() & 0o077 == 0
    }
}

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;
    use std::sync::atomic::Ordering;

    use super::{CAUGHT, LEFT};

    const CTRL_BREAK_EVENT: u32 = 1;
    const CTRL_CLOSE_EVENT: u32 = 2;
    const CTRL_LOGOFF_EVENT: u32 = 5;
    const CTRL_SHUTDOWN_EVENT: u32 = 6;

    #[link(name = "kernel32")]
    extern "system" {
        fn SetConsoleCtrlHandler(handler: Option<extern "system" fn(u32) -> i32>, add: i32) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn GetExitCodeProcess(process: *mut c_void, code: *mut u32) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    /// Runs on a thread of its own. A console closing ends the process a
    /// few seconds on, wherever its own thread is, so what it made goes
    /// here; a break asks it to end, as SIGTERM does. Ctrl+C comes as a key
    /// in raw mode, and the rest are the system's.
    extern "system" fn on_event(kind: u32) -> i32 {
        match kind {
            CTRL_CLOSE_EVENT | CTRL_LOGOFF_EVENT | CTRL_SHUTDOWN_EVENT => {
                CAUGHT.store(true, Ordering::SeqCst);
                LEFT.delete_all();
                1
            }
            CTRL_BREAK_EVENT => {
                CAUGHT.store(true, Ordering::SeqCst);
                1
            }
            _ => 0,
        }
    }

    pub fn catch() {
        // SAFETY: a handler that lives as long as the process.
        unsafe { SetConsoleCtrlHandler(Some(on_event), 1) };
    }

    /// Whether process `pid` runs, as far as the system says: one it won't
    /// open for us (another user's) runs, so what it left stays.
    pub fn running(pid: u32) -> bool {
        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        const STILL_ACTIVE: u32 = 259;
        // What opening a process that isn't there says.
        const ERROR_INVALID_PARAMETER: i32 = 87;
        // SAFETY: a handle opened only to ask, and closed after.
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return std::io::Error::last_os_error().raw_os_error() != Some(ERROR_INVALID_PARAMETER);
            }
            let mut code = 0;
            let asked = GetExitCodeProcess(process, &mut code);
            CloseHandle(process);
            asked == 0 || code == STILL_ACTIVE
        }
    }

    /// The temporary folder is in the user's own profile, where no one
    /// else may write, so a folder there is theirs.
    pub fn mine(_meta: &std::fs::Metadata) -> bool {
        true
    }
}

#[cfg(unix)]
pub use unix::{mine, running};
#[cfg(windows)]
pub use windows::{mine, running};

/// From here on, a hang-up (or SIGTERM, or on Windows the console closing
/// or a break) ends the demo as q does, deleting what it made.
pub fn catch() {
    #[cfg(unix)]
    unix::catch();
    #[cfg(windows)]
    windows::catch();
}

/// Whether process `pid` runs: where the system can't be asked, it does.
#[cfg(not(any(unix, windows)))]
pub fn running(_pid: u32) -> bool {
    true
}

#[cfg(not(any(unix, windows)))]
pub fn mine(_meta: &fs::Metadata) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of the test's own holding one file.
    fn scratch(name: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("flow-demo-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir(&dir).unwrap();
        let file = dir.join("balloon.glb");
        fs::write(&file, b"glTF").unwrap();
        (file, dir)
    }

    #[test]
    fn left_files_go_with_their_folders_and_forgotten_ones_stay() {
        let left = Left::new();
        let (file, dir) = scratch("left");
        let (kept_file, kept_dir) = scratch("left-kept");
        left.keep(&file, &dir);
        left.keep(&kept_file, &kept_dir);
        // The demo deleted that one itself, and someone made a new one.
        left.forget(&kept_dir);
        left.delete_all();
        assert!(!file.exists() && !dir.exists(), "the file and its folder are gone");
        assert!(kept_file.exists(), "one taken off the list is left alone");
        assert!(left.list().is_empty());
        let _ = fs::remove_dir_all(&kept_dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_hang_up_or_a_term_is_noted_and_the_demo_lives_on() {
        use std::ffi::c_int;
        extern "C" {
            fn raise(sig: c_int) -> c_int;
        }
        // Without the watcher, which would end the tests.
        unix::take_signals();
        for signum in [unix::SIGHUP, unix::SIGTERM] {
            CAUGHT.store(false, Ordering::SeqCst);
            // SAFETY: a signal to this thread, which the handler takes.
            assert_eq!(unsafe { raise(signum) }, 0);
            assert!(caught(), "signal {signum} noted");
            assert_eq!(unix::SIGNAL.load(Ordering::SeqCst), signum);
        }
    }

    #[cfg(unix)]
    #[test]
    fn once_woken_the_watcher_waits_its_grace_then_deletes_whats_left_and_ends() {
        use std::io::Write;
        use std::os::unix::net::UnixStream;
        use std::time::Instant;
        let left = Left::new();
        let (file, dir) = scratch("watched");
        left.keep(&file, &dir);
        let (mut wake, woken) = UnixStream::pair().unwrap();
        wake.write_all(&[1]).unwrap();
        let (start, mut ended) = (Instant::now(), false);
        unix::watch(woken, Duration::from_millis(50), &left, || ended = true);
        assert!(start.elapsed() >= Duration::from_millis(50), "the loop had its grace");
        assert!(ended && !file.exists() && !dir.exists());
        // A socket closed without a word ends nothing.
        let (wake, woken) = UnixStream::pair().unwrap();
        drop(wake);
        let mut ended = false;
        unix::watch(woken, Duration::ZERO, &left, || ended = true);
        assert!(!ended);
    }

    #[cfg(unix)]
    #[test]
    fn handing_the_terminal_back_waits_no_longer_than_its_moment() {
        use std::sync::Arc;
        use std::time::Instant;
        assert!(unix::within(Duration::from_secs(5), || {}), "what's done at once is done in time");
        // As the loop's write to a terminal that has stopped reading holds
        // std's lock on stdout, which handing the terminal back needs.
        let held = Arc::new(Mutex::new(()));
        let holding = held.lock().unwrap();
        let waiting = Arc::clone(&held);
        let start = Instant::now();
        assert!(!unix::within(Duration::from_millis(50), move || drop(waiting.lock())), "a held terminal isn't waited for");
        let took = start.elapsed();
        assert!(took >= Duration::from_millis(50) && took < Duration::from_secs(5), "it gave up after its wait, not before or long after: {took:?}");
        drop(holding);
    }

    #[cfg(unix)]
    #[test]
    fn the_system_says_which_processes_have_gone() {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        assert!(!running(pid), "a process that ended and was waited for is gone");
        assert!(running(std::process::id()));
        // The first process is another user's: it runs, as far as anyone knows.
        assert!(running(1));
        // Not one process: never taken as gone.
        assert!(running(0) && running(u32::MAX));
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_open_to_others_isnt_taken_for_the_demos() {
        use std::os::unix::fs::PermissionsExt;
        let (_, dir) = scratch("open");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(mine(&fs::symlink_metadata(&dir).unwrap()));
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(!mine(&fs::symlink_metadata(&dir).unwrap()));
        let _ = fs::remove_dir_all(&dir);
    }
}
