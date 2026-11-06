//! The library from C itself: the header's structs laid out as the crate's,
//! the settings' functions called from C, and the examples
//! (`examples/hello.c`, `examples/scene.c`, `examples/settings.c`) compiled
//! with `cc` against the shared library cargo built and run with no
//! terminal, where they have to fall back quietly and exit 0. Where there's
//! no `cc` (or on Windows, whose compilers take other flags) they say so and
//! pass.

mod common;

use std::fmt::Write as _;
use std::mem::{offset_of, size_of};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use flow_sdk_c::audio::FlowTone;
use flow_sdk_c::mesh::FlowMaterial;
use flow_sdk_c::model::FlowLook;
use flow_sdk_c::picture::FlowImage;
use flow_sdk_c::stage::FlowStageLayout;
use flow_sdk_c::term::{FlowCaps, FlowEvent, FlowSize};

/// A struct's name and size, and each of its fields' names and offsets.
type Layout<'a> = (&'a str, usize, &'a [(&'a str, usize)]);

/// Whether C can be built here, or why not.
fn can_build() -> Result<(), &'static str> {
    if cfg!(windows) {
        Err("not tried on Windows, whose compilers take other flags")
    } else if !common::have("cc") {
        Err("no cc on the PATH")
    } else {
        Ok(())
    }
}

/// Compiles `source` with the header, linking the shared library if
/// `link`, into the tests' scratch folder; the program's path.
fn compile(source: &Path, name: &str, link: bool) -> PathBuf {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let mut cc = Command::new("cc");
    cc.args(["-std=c99", "-Wall", "-Wextra"]).arg("-I").arg(common::root().join("include")).arg(source).arg("-o").arg(&out);
    if link {
        let library = common::library().expect("cargo builds the shared library for the tests");
        let dir = library.parent().expect("its folder");
        cc.arg("-L").arg(dir).arg("-lflow_sdk_c").arg(format!("-Wl,-rpath,{}", dir.display()));
    }
    let built = cc.stdin(Stdio::null()).output().expect("cc runs");
    assert!(built.status.success(), "cc failed on {}:\n{}", source.display(), String::from_utf8_lossy(&built.stderr));
    out
}

/// A program `compile` linked, to run with no terminal and on the library
/// it was linked against. Cargo puts the profile's folder on Linux's
/// `LD_LIBRARY_PATH`, which the loader searches before the program's own
/// runpath, and there a `cargo build -p flow-sdk-c` leaves the library as
/// it last built it: another version, or with or without audio.
fn run(program: &Path) -> Command {
    let mut command = Command::new(program);
    command.stdin(Stdio::null()).env_remove("LD_LIBRARY_PATH");
    command
}

#[test]
fn the_headers_structs_are_laid_out_as_the_crates() {
    if let Err(why) = can_build() {
        eprintln!("skipped: {why}");
        return;
    }
    // Each struct's size and each field's offset, as C has them and as Rust does.
    let fields: [Layout; 8] = [
        ("FlowSize", size_of::<FlowSize>(), &[("cols", offset_of!(FlowSize, cols)), ("rows", offset_of!(FlowSize, rows)), ("cell_width", offset_of!(FlowSize, cell_width)), ("cell_height", offset_of!(FlowSize, cell_height))]),
        (
            "FlowCaps",
            size_of::<FlowCaps>(),
            &[
                ("pictures", offset_of!(FlowCaps, pictures)),
                ("models", offset_of!(FlowCaps, models)),
                ("flow", offset_of!(FlowCaps, flow)),
                ("bare", offset_of!(FlowCaps, bare)),
                ("piped", offset_of!(FlowCaps, piped)),
                ("cell_width", offset_of!(FlowCaps, cell_width)),
                ("cell_height", offset_of!(FlowCaps, cell_height)),
                ("area_width", offset_of!(FlowCaps, area_width)),
                ("area_height", offset_of!(FlowCaps, area_height)),
            ],
        ),
        (
            "FlowEvent",
            size_of::<FlowEvent>(),
            &[("kind", offset_of!(FlowEvent, kind)), ("key", offset_of!(FlowEvent, key)), ("mods", offset_of!(FlowEvent, mods)), ("col", offset_of!(FlowEvent, col)), ("row", offset_of!(FlowEvent, row)), ("button", offset_of!(FlowEvent, button))],
        ),
        (
            "FlowImage",
            size_of::<FlowImage>(),
            &[("format", offset_of!(FlowImage, format)), ("width", offset_of!(FlowImage, width)), ("height", offset_of!(FlowImage, height)), ("data", offset_of!(FlowImage, data)), ("len", offset_of!(FlowImage, len))],
        ),
        (
            "FlowLook",
            size_of::<FlowLook>(),
            &[
                ("animate", offset_of!(FlowLook, animate)),
                ("color", offset_of!(FlowLook, color)),
                ("scale", offset_of!(FlowLook, scale)),
                ("depth", offset_of!(FlowLook, depth)),
                ("brightness", offset_of!(FlowLook, brightness)),
                ("offset", offset_of!(FlowLook, offset)),
                ("rotation", offset_of!(FlowLook, rotation)),
                ("scale3", offset_of!(FlowLook, scale3)),
            ],
        ),
        (
            "FlowMaterial",
            size_of::<FlowMaterial>(),
            &[
                ("color", offset_of!(FlowMaterial, color)),
                ("gloss", offset_of!(FlowMaterial, gloss)),
                ("metal", offset_of!(FlowMaterial, metal)),
                ("glow", offset_of!(FlowMaterial, glow)),
                ("glow_strength", offset_of!(FlowMaterial, glow_strength)),
                ("coat", offset_of!(FlowMaterial, coat)),
                ("coat_gloss", offset_of!(FlowMaterial, coat_gloss)),
                ("unlit", offset_of!(FlowMaterial, unlit)),
            ],
        ),
        (
            "FlowStageLayout",
            size_of::<FlowStageLayout>(),
            &[
                ("left", offset_of!(FlowStageLayout, left)),
                ("top", offset_of!(FlowStageLayout, top)),
                ("cols", offset_of!(FlowStageLayout, cols)),
                ("rows", offset_of!(FlowStageLayout, rows)),
                ("below", offset_of!(FlowStageLayout, below)),
                ("width", offset_of!(FlowStageLayout, width)),
                ("height", offset_of!(FlowStageLayout, height)),
            ],
        ),
        (
            "FlowTone",
            size_of::<FlowTone>(),
            &[
                ("hz", offset_of!(FlowTone, hz)),
                ("secs", offset_of!(FlowTone, secs)),
                ("wave", offset_of!(FlowTone, wave)),
                ("volume", offset_of!(FlowTone, volume)),
                ("pan", offset_of!(FlowTone, pan)),
                ("attack", offset_of!(FlowTone, attack)),
                ("release", offset_of!(FlowTone, release)),
            ],
        ),
    ];
    let mut source = String::from("#include <stdio.h>\n#include <stddef.h>\n#include \"flow_sdk.h\"\nint main(void) {\n");
    let mut expected = String::new();
    for (name, size, members) in fields {
        let _ = writeln!(source, "    printf(\"{name} %zu\\n\", sizeof({name}));");
        let _ = writeln!(expected, "{name} {size}");
        for (field, offset) in members {
            let _ = writeln!(source, "    printf(\"{name}.{field} %zu\\n\", offsetof({name}, {field}));");
            let _ = writeln!(expected, "{name}.{field} {offset}");
        }
    }
    source.push_str("    return 0;\n}\n");
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flow_sdk_layout.c");
    std::fs::write(&path, source).expect("the scratch folder");
    let program = compile(&path, "flow_sdk_layout", false);
    let ran = Command::new(program).stdin(Stdio::null()).output().expect("it runs");
    assert_eq!(String::from_utf8_lossy(&ran.stdout), expected);
}

#[test]
fn the_c_example_builds_against_the_library_and_falls_back_quietly_outside_a_terminal() {
    if let Err(why) = can_build() {
        eprintln!("skipped: {why}");
        return;
    }
    let program = compile(&common::root().join("examples/hello.c"), "flow_sdk_hello", true);
    // No terminal: input from nothing and output to a pipe, and not known
    // as Flow by name either, as when the tests run in one of its tabs.
    let ran = run(&program).env_remove("TERM_PROGRAM").output().expect("it runs");
    let said = String::from_utf8_lossy(&ran.stdout);
    assert!(ran.status.success(), "it exited with {}:\n{said}{}", ran.status, String::from_utf8_lossy(&ran.stderr));
    assert!(said.contains("the gem (8 triangles,") && said.contains("goes unseen"), "{said}");
    assert!(!said.contains('\x1b'), "nothing asked or sent outside a terminal: {said:?}");
}

#[test]
fn the_scene_example_builds_and_without_a_terminal_says_what_goes_unseen_and_unheard() {
    if let Err(why) = can_build() {
        eprintln!("skipped: {why}");
        return;
    }
    let program = compile(&common::root().join("examples/scene.c"), "flow_sdk_scene", true);
    let ran = run(&program).env_remove("TERM_PROGRAM").output().expect("it runs");
    let said = String::from_utf8_lossy(&ran.stdout);
    assert!(ran.status.success(), "it exited with {}:\n{said}{}", ran.status, String::from_utf8_lossy(&ran.stderr));
    assert!(said.contains("the cube (6 triangles") && said.contains("goes unseen"), "{said}");
    // Its song is made and timed without a sound played: eight beats at 150
    // beats a minute.
    let heard = if cfg!(feature = "audio") { "its dance (3.2 s) unheard" } else { "no audio in this build" };
    assert!(said.contains(heard), "{said}");
    assert!(!said.contains('\x1b'), "nothing asked or sent outside a terminal: {said:?}");
}

#[test]
fn the_settings_are_asked_from_c_as_the_sdk_asks_and_answers_read_back() {
    if let Err(why) = can_build() {
        eprintln!("skipped: {why}");
        return;
    }
    let source = r#"#include <stdio.h>
#include <string.h>
#include "flow_sdk.h"
static void say(const char *what, char *text) {
    printf("%s %s\n", what, text ? text : "NULL");
    flow_free(text);
}
int main(void) {
    say("request", flow_settings_request("{\"do\":\"get\",\"key\":\"fx.style\"}"));
    const char reply[] = "x\x1b]7732;eyJvayI6dHJ1ZX0=\x1b\\y";
    say("answer", flow_settings_answer(reply, strlen(reply)));
    say("half", flow_settings_answer(reply, 12));
    say("other", flow_settings_answer("\x1b]11;rgb:0/0/0\x07", 15));
    say("refused", flow_settings_request(NULL));
    say("asked", flow_settings_ask("{\"do\":\"list\"}", 5000));
    return 0;
}
"#;
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flow_sdk_settings_api.c");
    std::fs::write(&path, source).expect("the scratch folder");
    let program = compile(&path, "flow_sdk_settings_api", true);
    // Run once untimed: macOS holds the first start of a program or library
    // just built while it checks it, seconds on a busy machine.
    run(&program).env_remove(flow_sdk::pipe::VAR).output().expect("it runs");
    let started = Instant::now();
    let ran = run(&program).env_remove(flow_sdk::pipe::VAR).output().expect("it runs");
    assert!(ran.status.success(), "it exited with {}:\n{}", ran.status, String::from_utf8_lossy(&ran.stderr));
    // With no terminal, asking asks nothing and waits for nothing.
    let expected = "request \x1b]7732;eyJkbyI6ImdldCIsImtleSI6ImZ4LnN0eWxlIn0=\x07\nanswer {\"ok\":true}\nhalf NULL\nother NULL\nrefused NULL\nasked NULL\n";
    assert_eq!(String::from_utf8_lossy(&ran.stdout), expected);
    assert!(started.elapsed() < Duration::from_secs(3), "it took {:?}", started.elapsed());
}

#[test]
fn the_settings_example_asks_only_in_flow_and_says_when_nothing_answered() {
    if let Err(why) = can_build() {
        eprintln!("skipped: {why}");
        return;
    }
    let program = compile(&common::root().join("examples/settings.c"), "flow_sdk_settings", true);
    let elsewhere = run(&program).env_remove("TERM_PROGRAM").env_remove(flow_sdk::pipe::VAR).output().expect("it runs");
    let said = String::from_utf8_lossy(&elsewhere.stdout);
    assert!(elsewhere.status.success() && said.contains("isn't a Flow tab"), "{said}");
    // Named as Flow but with no terminal to ask: nothing asked, at once.
    let started = Instant::now();
    let named = run(&program).env("TERM_PROGRAM", "flow-terminal").env_remove(flow_sdk::pipe::VAR).output().expect("it runs");
    let said = String::from_utf8_lossy(&named.stdout);
    assert!(named.status.success() && said.contains("Nothing answered"), "{said}{}", String::from_utf8_lossy(&named.stderr));
    assert!(started.elapsed() < Duration::from_secs(3), "it took {:?}", started.elapsed());
    assert!(!said.contains('\x1b'), "nothing asked outside a terminal: {said:?}");
    let refused = run(&program).arg("rain\"}").env_remove("TERM_PROGRAM").output().expect("it runs");
    assert_eq!(refused.status.code(), Some(2), "a name that would end the JSON");
}
