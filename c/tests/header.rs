//! The header is written by hand, so this keeps it in step with the crate:
//! every function the crate exports (`#[no_mangle]`) is declared in
//! `include/flow_sdk.h` under the same name and none other is, and the
//! header's numbers (its `#define`s, negative ones too) are the crate's,
//! its rotation that doesn't turn the SDK's too.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use flow_sdk::fx::Sound;
use flow_sdk_c::{audio, fx, model, picture, term, theme, ABI};

fn header() -> String {
    std::fs::read_to_string(common::root().join("include/flow_sdk.h")).expect("the header")
}

/// The functions the crate's sources mark `#[no_mangle]`.
fn exported() -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let src = common::root().join("src");
    for entry in std::fs::read_dir(&src).expect("src/") {
        let path = entry.expect("a file in src/").path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a source file");
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.trim() != "#[no_mangle]" {
                continue;
            }
            let signature = lines[i + 1..].iter().find(|l| l.contains(" fn ")).unwrap_or_else(|| panic!("a function after line {} of {}", i + 1, path.display()));
            let after = signature.split(" fn ").nth(1).expect("a name");
            names.insert(after[..after.find('(').expect("its arguments")].to_string());
        }
    }
    names
}

/// The header without its comments.
fn code(header: &str) -> String {
    let mut out = String::new();
    let mut rest = header;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        rest = rest[start..].find("*/").map_or("", |end| &rest[start + end + 2..]);
    }
    out.push_str(rest);
    out.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n")
}

/// The functions the header declares: each `flow_…` name followed by its
/// arguments.
fn declared() -> BTreeSet<String> {
    let code = code(&header());
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut names = BTreeSet::new();
    for (at, _) in code.match_indices("flow_") {
        if code[..at].chars().next_back().is_some_and(ident) {
            continue;
        }
        let name: String = code[at..].chars().take_while(|&c| ident(c)).collect();
        if code[at + name.len()..].trim_start().starts_with('(') {
            names.insert(name);
        }
    }
    names
}

/// The header's `#define FLOW_… <number>`s.
fn defines() -> BTreeMap<String, i64> {
    let mut found = BTreeMap::new();
    for line in code(&header()).lines() {
        let mut words = line.split_whitespace();
        if words.next() != Some("#define") {
            continue;
        }
        let (Some(name), Some(value)) = (words.next(), words.next()) else { continue };
        let value = value.trim_end_matches('u');
        let number = match value.strip_prefix("0x") {
            Some(hex) => i64::from_str_radix(hex, 16).ok(),
            None => value.parse().ok(),
        };
        if let Some(n) = number {
            found.insert(name.to_string(), n);
        }
    }
    found
}

#[test]
fn every_exported_function_is_declared_in_the_header_and_no_other() {
    let exported = exported();
    let declared = declared();
    assert!(exported.len() > 50, "found only {exported:?}");
    let missing: Vec<_> = exported.difference(&declared).collect();
    let extra: Vec<_> = declared.difference(&exported).collect();
    assert!(missing.is_empty(), "exported but not in flow_sdk.h: {missing:?}");
    assert!(extra.is_empty(), "in flow_sdk.h but not exported: {extra:?}");
}

#[test]
fn the_headers_numbers_are_the_crates() {
    let defines = defines();
    let mut expected: Vec<(&str, i64)> = vec![
        ("FLOW_SDK_ABI", i64::from(ABI)),
        ("FLOW_ASK_PICTURES", term::ASK_PICTURES.into()),
        ("FLOW_ASK_MODELS", term::ASK_MODELS.into()),
        ("FLOW_ASK_CELL", term::ASK_CELL.into()),
        ("FLOW_ASK_AREA", term::ASK_AREA.into()),
        ("FLOW_ASK_VERSION", term::ASK_VERSION.into()),
        ("FLOW_ASK_NO_PIPE", term::ASK_NO_PIPE.into()),
        ("FLOW_ASK_BACKGROUND", term::ASK_BACKGROUND.into()),
        ("FLOW_ASK_EVERYTHING", (term::ASK_PICTURES | term::ASK_MODELS | term::ASK_CELL | term::ASK_AREA | term::ASK_VERSION | term::ASK_BACKGROUND).into()),
        ("FLOW_EVENT_KEY", term::EVENT_KEY as i64),
        ("FLOW_EVENT_CLICK", term::EVENT_CLICK as i64),
        ("FLOW_EVENT_RESIZE", term::EVENT_RESIZE as i64),
        ("FLOW_EVENT_OTHER", term::EVENT_OTHER as i64),
        ("FLOW_KEY_TAB", term::KEY_TAB.into()),
        ("FLOW_KEY_ENTER", term::KEY_ENTER.into()),
        ("FLOW_KEY_ESCAPE", term::KEY_ESCAPE.into()),
        ("FLOW_KEY_BACKSPACE", term::KEY_BACKSPACE.into()),
        ("FLOW_KEY_OTHER", term::KEY_OTHER.into()),
        ("FLOW_KEY_LEFT", term::KEY_LEFT.into()),
        ("FLOW_KEY_RIGHT", term::KEY_RIGHT.into()),
        ("FLOW_KEY_UP", term::KEY_UP.into()),
        ("FLOW_KEY_DOWN", term::KEY_DOWN.into()),
        ("FLOW_KEY_HOME", term::KEY_HOME.into()),
        ("FLOW_KEY_END", term::KEY_END.into()),
        ("FLOW_KEY_PAGE_UP", term::KEY_PAGE_UP.into()),
        ("FLOW_KEY_PAGE_DOWN", term::KEY_PAGE_DOWN.into()),
        ("FLOW_KEY_DELETE", term::KEY_DELETE.into()),
        ("FLOW_KEY_INSERT", term::KEY_INSERT.into()),
        ("FLOW_KEY_BACKTAB", term::KEY_BACKTAB.into()),
        ("FLOW_KEY_F1", term::KEY_F1.into()),
        ("FLOW_MOD_SHIFT", term::MOD_SHIFT.into()),
        ("FLOW_MOD_CTRL", term::MOD_CTRL.into()),
        ("FLOW_MOD_ALT", term::MOD_ALT.into()),
        ("FLOW_PIXELS_RGB", picture::PIXELS_RGB as i64),
        ("FLOW_PIXELS_RGBA", picture::PIXELS_RGBA as i64),
        ("FLOW_PIXELS_PNG", picture::PIXELS_PNG as i64),
        ("FLOW_PLACEMENT", flow_sdk::picture::PLACEMENT.into()),
        ("FLOW_MODEL_OBJ", model::MODEL_OBJ as i64),
        ("FLOW_MODEL_GLB", model::MODEL_GLB as i64),
        ("FLOW_FX_BOLT", fx::FX_BOLT as i64),
        ("FLOW_FX_BURST", fx::FX_BURST as i64),
        ("FLOW_FX_RIPPLE", fx::FX_RIPPLE as i64),
        ("FLOW_FX_STIR", fx::FX_STIR as i64),
        ("FLOW_WAVE_SINE", audio::WAVE_SINE as i64),
        ("FLOW_WAVE_TRIANGLE", audio::WAVE_TRIANGLE as i64),
        ("FLOW_WAVE_SQUARE", audio::WAVE_SQUARE as i64),
        ("FLOW_WAVE_SAW", audio::WAVE_SAW as i64),
        ("FLOW_WAVE_NOISE", audio::WAVE_NOISE as i64),
        ("FLOW_INSTRUMENT_LUTE", audio::INSTRUMENT_LUTE as i64),
        ("FLOW_INSTRUMENT_RECORDER", audio::INSTRUMENT_RECORDER as i64),
        ("FLOW_INSTRUMENT_DRONE", audio::INSTRUMENT_DRONE as i64),
        ("FLOW_INSTRUMENT_TABOR", audio::INSTRUMENT_TABOR as i64),
        ("FLOW_THEME_FAILED", theme::FAILED.into()),
    ];
    let sounds: Vec<String> = Sound::ALL.iter().map(|s| format!("FLOW_SOUND_{}", s.name().to_uppercase())).collect();
    expected.extend(sounds.iter().enumerate().map(|(i, name)| (name.as_str(), i as i64)));
    for (name, value) in &expected {
        assert_eq!(defines.get(*name), Some(value), "{name}");
    }
    // And the header names no number the crate doesn't.
    let known: BTreeSet<&str> = expected.iter().map(|(name, _)| *name).collect();
    let unchecked: Vec<&String> = defines.keys().filter(|d| !known.contains(d.as_str())).collect();
    assert!(unchecked.is_empty(), "#defines with no constant to check against: {unchecked:?}");
}

#[test]
fn the_headers_identity_is_the_sdks() {
    let code = code(&header());
    let line = code.lines().find(|l| l.starts_with("#define FLOW_M3_IDENTITY ")).expect("FLOW_M3_IDENTITY");
    let inside = line.split_once('{').and_then(|(_, rest)| rest.split_once('}')).map(|(inside, _)| inside).expect("its braces");
    let numbers: Vec<f32> = inside.split(',').map(|n| n.trim().trim_end_matches('f').parse().expect("a number")).collect();
    let rows: Vec<f32> = flow_sdk::scene::IDENTITY.iter().flatten().copied().collect();
    assert_eq!(numbers, rows);
}
