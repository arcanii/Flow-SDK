//! The Python package (`python/`) against the shared library cargo built
//! for these tests: its own unittest tests, run with `python3 -I` (and `-B`,
//! leaving no bytecode in the tree) and `FLOW_SDK_LIB` naming the library,
//! with no terminal (nor Flow's pipe, so nothing answers Flow's settings)
//! but the PTY a settings test makes of its own, which answers as Flow.
//! Built without audio (`--no-default-features`), the tests that mix sound
//! skip, saying why, and the rest still run. Where there's no python3 it
//! says so and passes, and so it does where the package isn't beside the
//! crate: in the crate as `cargo package` makes it, which holds only what's
//! in c/.

mod common;

use std::process::{Command, Stdio};

#[test]
fn the_python_package_passes_its_tests_against_the_library() {
    if !common::have("python3") {
        eprintln!("skipped: no python3 on the PATH");
        return;
    }
    let tests = common::root().join("../python/tests");
    if !tests.join("test_flow_sdk.py").is_file() {
        eprintln!("skipped: the Python package isn't beside the crate here ({})", tests.display());
        return;
    }
    let library = common::library().expect("cargo builds the shared library for the tests");
    let ran = Command::new("python3")
        .args(["-I", "-B", "-m", "unittest", "discover", "-v", "-s"])
        .arg(&tests)
        .env("FLOW_SDK_LIB", &library)
        .env_remove("TERM_PROGRAM")
        .env_remove(flow_sdk::pipe::VAR)
        .stdin(Stdio::null())
        .output()
        .expect("python3 runs");
    // unittest reports on standard error.
    let report = String::from_utf8_lossy(&ran.stderr);
    assert!(ran.status.success(), "the Python tests failed:\n{}{report}", String::from_utf8_lossy(&ran.stdout));
    // Every test runs with the library named, but for those that mix sound
    // in a library without it, and on Windows those that answer Flow's
    // settings as keys in a PTY, since there the answer comes down its pipe.
    let skipped: Vec<&str> = report.lines().filter(|l| l.contains("skipped")).filter(|l| !l.starts_with("OK")).collect();
    let allowed = |line: &&str| {
        (!cfg!(feature = "audio") && line.contains("the library has no audio")) || (cfg!(windows) && line.contains("on Windows the answer comes down Flow's pipe"))
    };
    assert!(skipped.iter().all(allowed), "tests skipped:\n{report}");
}
