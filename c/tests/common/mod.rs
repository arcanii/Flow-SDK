//! What the integration tests share: where the shared library they test
//! was built, and whether a tool is on the PATH.

// Each test file takes the few of these it needs.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The crate's root, where `include/` and `examples/` are.
pub fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The shared library's file name on this OS.
pub fn library_name() -> String {
    format!("{}flow_sdk_c{}", std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX)
}

/// The shared library cargo built for these tests: beside the test's own
/// binary in `deps/`, or in the profile's folder above it.
pub fn library() -> Option<PathBuf> {
    let deps = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let name = library_name();
    [deps.join(&name), deps.parent()?.join(&name)].into_iter().find(|p| p.is_file())
}

/// Whether `tool --version` runs.
pub fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").stdin(Stdio::null()).output().is_ok_and(|o| o.status.success())
}
