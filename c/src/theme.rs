//! Theme packages for Flow from C ([`flow_sdk::theme`]): a `FlowTheme`
//! built a setting at a time, `FlowThemePackage`s of them, checked as Flow
//! checks them, written into a folder or packed as the `.tar.gz` that
//! `flowapp install` takes. The cursor's shape, the CRT preset and the
//! sounds go by the names `themes.json` gives them ("bar", "classic",
//! "terminal"), refused (-1) where Flow has none; colors are 0xRRGGBB.
//!
//! What only the whole package can tell (an id Flow won't take, a font
//! file that isn't there or isn't a font, two files for one place) is said
//! by `flow_theme_check`, `flow_theme_package_check` and the functions that
//! write, as a reason in `*error` to free with `flow_free`, as
//! `flow_tune_new` says why notes don't read: -1 for what Flow would
//! refuse, [`FAILED`] (-2) for a file that couldn't be read or written.

use std::ffi::{c_char, c_int};

use flow_sdk::theme::{serde_json, Crt, CursorShape, Error, Package, Sounds, Theme};

use crate::{guard, owned, owned_or_null, slice, tell, text, untold};

/// A theme being built: `flow_theme_new` to `flow_theme_free`.
pub struct FlowTheme(pub(crate) Theme);

/// A package of themes: `flow_theme_package_new` to
/// `flow_theme_package_free`.
pub struct FlowThemePackage(pub(crate) Package);

/// Changes the theme behind `theme` by `change`: 0, or -1 for NULL or a
/// value refused.
unsafe fn with(theme: *mut FlowTheme, change: impl FnOnce(Theme) -> Option<Theme>) -> c_int {
    guard(-1, || {
        let Some(theme) = theme.as_mut() else { return -1 };
        match change(theme.0.clone()) {
            Some(changed) => {
                theme.0 = changed;
                0
            }
            None => -1,
        }
    })
}

/// `count` strings at `strings`, each UTF-8; None if any is NULL or isn't.
unsafe fn strings(strings: *const *const c_char, count: usize) -> Option<Vec<String>> {
    slice(strings, count)?.iter().map(|s| text(*s).map(String::from)).collect()
}

/// What a function with an `error` returns where a file couldn't be read
/// or written, or tar failed: -1 is for what Flow would refuse.
pub const FAILED: c_int = -2;

/// Runs `body`, which may fail saying why: 0, or -1 (refused) or
/// [`FAILED`] with the reason in `*error`.
unsafe fn told(error: *mut *mut c_char, body: impl FnOnce() -> Result<(), Error>) -> c_int {
    untold(error);
    let done = guard(Err(Error::Refused("Flow's library failed inside".into())), body);
    match done {
        Ok(()) => 0,
        Err(why) => {
            tell(error, &why.to_string());
            if matches!(why, Error::Io(..)) {
                FAILED
            } else {
                -1
            }
        }
    }
}

/// A refusal of what was passed in.
fn passed(why: &str) -> Error {
    Error::Refused(why.into())
}

/// A theme with id `id`, wearing Flow's own colors until given its own;
/// NULL for NULL or text that isn't UTF-8. Its id is checked with the rest.
#[no_mangle]
pub unsafe extern "C" fn flow_theme_new(id: *const c_char) -> *mut FlowTheme {
    guard(std::ptr::null_mut(), || match text(id) {
        Some(id) => Box::into_raw(Box::new(FlowTheme(Theme::new(id)))),
        None => std::ptr::null_mut(),
    })
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_free(theme: *mut FlowTheme) {
    if !theme.is_null() {
        guard((), || drop(Box::from_raw(theme)));
    }
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_name(theme: *mut FlowTheme, name: *const c_char) -> c_int {
    with(theme, |t| Some(t.name(text(name)?)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_name_in(theme: *mut FlowTheme, code: *const c_char, name: *const c_char) -> c_int {
    with(theme, |t| Some(t.name_in(text(code)?, text(name)?)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_fonts(theme: *mut FlowTheme, families: *const *const c_char, count: usize) -> c_int {
    with(theme, |t| Some(t.fonts(strings(families, count)?)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_font_file(theme: *mut FlowTheme, path: *const c_char) -> c_int {
    with(theme, |t| Some(t.font_file(text(path)?)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_text(theme: *mut FlowTheme, rgb: u32) -> c_int {
    with(theme, |t| Some(t.text(rgb)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_cursor(theme: *mut FlowTheme, rgb: u32) -> c_int {
    with(theme, |t| Some(t.cursor(rgb)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_background(theme: *mut FlowTheme, rgb: u32) -> c_int {
    with(theme, |t| Some(t.background(rgb)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_border(theme: *mut FlowTheme, rgb: u32) -> c_int {
    with(theme, |t| Some(t.border(rgb)))
}

/// Sixteen colors, no fewer or more.
#[no_mangle]
pub unsafe extern "C" fn flow_theme_ansi(theme: *mut FlowTheme, colors: *const u32, count: usize) -> c_int {
    with(theme, |t| Some(t.ansi(slice(colors, count)?.try_into().ok()?)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_phosphor(theme: *mut FlowTheme, rgb: u32) -> c_int {
    with(theme, |t| Some(t.phosphor(rgb)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_cursor_shape(theme: *mut FlowTheme, shape: *const c_char) -> c_int {
    with(theme, |t| Some(t.cursor_shape(CursorShape::named(text(shape)?)?)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_blink(theme: *mut FlowTheme, blink: bool) -> c_int {
    with(theme, |t| Some(t.blink(blink)))
}

/// 0.5 to 4; a number outside that is refused at once, since Flow would
/// change it.
#[no_mangle]
pub unsafe extern "C" fn flow_theme_cursor_speed(theme: *mut FlowTheme, speed: f32) -> c_int {
    with(theme, |t| (0.5..=4.0).contains(&speed).then(|| t.cursor_speed(speed)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_crt(theme: *mut FlowTheme, preset: *const c_char) -> c_int {
    with(theme, |t| Some(t.crt(Crt::named(text(preset)?)?)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_start(theme: *mut FlowTheme, lines: *const *const c_char, count: usize) -> c_int {
    with(theme, |t| Some(t.start(strings(lines, count)?)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_baud(theme: *mut FlowTheme, baud: u32) -> c_int {
    with(theme, |t| Some(t.baud(baud)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_prompt(theme: *mut FlowTheme, prompt: *const c_char) -> c_int {
    with(theme, |t| Some(t.prompt(text(prompt)?)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_sounds(theme: *mut FlowTheme, sounds: *const c_char) -> c_int {
    with(theme, |t| Some(t.sounds(Sounds::named(text(sounds)?)?)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_screen(theme: *mut FlowTheme, cols: u16, rows: u16) -> c_int {
    with(theme, |t| Some(t.screen(cols.into(), rows.into())))
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_logo(theme: *mut FlowTheme, path: *const c_char) -> c_int {
    with(theme, |t| Some(t.logo(text(path)?)))
}

/// `key` of `section`, one of the look's settings, as `json` (a JSON
/// value, as text): refused at once, saying why, where it isn't JSON, isn't
/// one a theme may bring, or isn't of its kind.
#[no_mangle]
pub unsafe extern "C" fn flow_theme_key(theme: *mut FlowTheme, section: *const c_char, key: *const c_char, json: *const c_char, error: *mut *mut c_char) -> c_int {
    told(error, || {
        let Some(theme) = theme.as_mut() else { return Err(passed("the theme is NULL")) };
        let (Some(section), Some(key), Some(json)) = (text(section), text(key), text(json)) else { return Err(passed("a NULL or text that isn't UTF-8")) };
        let value: serde_json::Value = serde_json::from_str(json).map_err(|e| Error::Refused(format!("{section}.{key}: {e}")))?;
        // Tried alone first, so what's wrong is said now.
        Theme::new("key").key(section, key, value.clone()).check().map_err(|e| Error::Refused(e.to_string().trim_start_matches("key: ").to_string()))?;
        theme.0 = theme.0.clone().key(section, key, value);
        Ok(())
    })
}

/// Whether Flow would take the theme, and the files it brings are there
/// and are what they say ([`FAILED`] for one that can't be read).
#[no_mangle]
pub unsafe extern "C" fn flow_theme_check(theme: *const FlowTheme, error: *mut *mut c_char) -> c_int {
    told(error, || theme.as_ref().ok_or_else(|| passed("the theme is NULL"))?.0.check())
}

/// A package called `name`, at `version`, holding no themes yet; NULL for
/// NULL or text that isn't UTF-8. Both are checked with the rest.
#[no_mangle]
pub unsafe extern "C" fn flow_theme_package_new(name: *const c_char, version: *const c_char) -> *mut FlowThemePackage {
    guard(std::ptr::null_mut(), || match (text(name), text(version)) {
        (Some(name), Some(version)) => Box::into_raw(Box::new(FlowThemePackage(Package::new(name, version)))),
        _ => std::ptr::null_mut(),
    })
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_package_free(package: *mut FlowThemePackage) {
    if !package.is_null() {
        guard((), || drop(Box::from_raw(package)));
    }
}

/// Adds a copy of `theme`, in place of one with its id already there; the
/// theme stays the caller's to free.
#[no_mangle]
pub unsafe extern "C" fn flow_theme_package_add(package: *mut FlowThemePackage, theme: *const FlowTheme) -> c_int {
    guard(-1, || {
        let (Some(package), Some(theme)) = (package.as_mut(), theme.as_ref()) else { return -1 };
        package.0.add(theme.0.clone());
        0
    })
}

#[no_mangle]
pub unsafe extern "C" fn flow_theme_package_check(package: *const FlowThemePackage, error: *mut *mut c_char) -> c_int {
    told(error, || package.as_ref().ok_or_else(|| passed("the package is NULL"))?.0.check())
}

/// Its `themes.json`, checked; NULL, saying why in `*error`, if it isn't
/// one Flow would take.
#[no_mangle]
pub unsafe extern "C" fn flow_theme_package_json(package: *const FlowThemePackage, error: *mut *mut c_char) -> *mut c_char {
    let mut json = None;
    let done = told(error, || {
        json = Some(package.as_ref().ok_or_else(|| passed("the package is NULL"))?.0.json()?);
        Ok(())
    });
    match json {
        Some(json) if done == 0 => owned(json),
        _ => std::ptr::null_mut(),
    }
}

/// `<name>-<version>.tar.gz`; NULL for NULL.
#[no_mangle]
pub unsafe extern "C" fn flow_theme_package_file_name(package: *const FlowThemePackage) -> *mut c_char {
    owned_or_null(|| Some(package.as_ref()?.0.file_name()))
}

/// Writes it, checked, into the folder `dir` (made if it isn't there).
#[no_mangle]
pub unsafe extern "C" fn flow_theme_package_write(package: *const FlowThemePackage, dir: *const c_char, error: *mut *mut c_char) -> c_int {
    told(error, || {
        let package = package.as_ref().ok_or_else(|| passed("the package is NULL"))?;
        let dir = text(dir).ok_or_else(|| passed("the folder is NULL, or isn't UTF-8"))?;
        package.0.write(dir)
    })
}

/// Packs it, checked, as the `.tar.gz` `tarball`, with the system's tar.
#[no_mangle]
pub unsafe extern "C" fn flow_theme_package_pack(package: *const FlowThemePackage, tarball: *const c_char, error: *mut *mut c_char) -> c_int {
    told(error, || {
        let package = package.as_ref().ok_or_else(|| passed("the package is NULL"))?;
        let tarball = text(tarball).ok_or_else(|| passed("the file is NULL, or isn't UTF-8"))?;
        package.0.pack(tarball)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

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
    fn a_theme_built_from_c_is_written_as_the_sdks() {
        unsafe {
            let theme = flow_theme_new(c"vt52".as_ptr());
            assert_eq!(flow_theme_name(theme, c"VT52".as_ptr()), 0);
            assert_eq!(flow_theme_name_in(theme, c"ja".as_ptr(), c"VT52 端末".as_ptr()), 0);
            let fonts = [c"VT323".as_ptr(), c"Glass TTY VT220".as_ptr()];
            assert_eq!(flow_theme_fonts(theme, fonts.as_ptr(), fonts.len()), 0);
            assert_eq!(flow_theme_text(theme, 0xE8F0F0), 0);
            assert_eq!(flow_theme_phosphor(theme, 0xE8F0F0), 0);
            assert_eq!(flow_theme_cursor_shape(theme, c"bar".as_ptr()), 0);
            assert_eq!(flow_theme_crt(theme, c"classic".as_ptr()), 0);
            let lines = [c"".as_ptr(), c"^READY".as_ptr()];
            assert_eq!(flow_theme_start(theme, lines.as_ptr(), lines.len()), 0);
            assert_eq!(flow_theme_baud(theme, 9600), 0);
            assert_eq!(flow_theme_prompt(theme, c"$ ".as_ptr()), 0);
            assert_eq!(flow_theme_sounds(theme, c"terminal".as_ptr()), 0);
            assert_eq!(flow_theme_screen(theme, 80, 24), 0);
            let mut error: *mut c_char = std::ptr::null_mut();
            assert_eq!(flow_theme_key(theme, c"fx".as_ptr(), c"sun".as_ptr(), c"0.5".as_ptr(), &mut error), 0);
            assert!(error.is_null());
            let sdk = Theme::new("vt52")
                .name("VT52")
                .name_in("ja", "VT52 端末")
                .fonts(["VT323", "Glass TTY VT220"])
                .text(0xE8F0F0)
                .phosphor(0xE8F0F0)
                .cursor_shape(CursorShape::Bar)
                .crt(Crt::Classic)
                .start(["", "^READY"])
                .baud(9600)
                .prompt("$ ")
                .sounds(Sounds::Terminal)
                .screen(80, 24)
                .key("fx", "sun", 0.5);
            assert_eq!((*theme).0, sdk);
            let package = flow_theme_package_new(c"vt-themes".as_ptr(), c"1.0".as_ptr());
            assert_eq!(flow_theme_package_add(package, theme), 0);
            flow_theme_free(theme);
            let json = take(flow_theme_package_json(package, &mut error)).expect("checked");
            assert!(error.is_null());
            assert_eq!(json, Package::new("vt-themes", "1.0").theme(sdk).json().unwrap());
            assert_eq!(take(flow_theme_package_file_name(package)).as_deref(), Some("vt-themes-1.0.tar.gz"));
            flow_theme_package_free(package);
        }
    }

    #[test]
    fn what_flow_hasnt_is_refused_at_once_and_what_only_the_whole_can_tell_says_why() {
        unsafe {
            let theme = flow_theme_new(c"mine".as_ptr());
            assert_eq!(flow_theme_cursor_shape(theme, c"heart".as_ptr()), -1);
            assert_eq!(flow_theme_crt(theme, c"mine".as_ptr()), -1);
            assert_eq!(flow_theme_sounds(theme, c"organ".as_ptr()), -1);
            assert_eq!(flow_theme_cursor_speed(theme, 9.0), -1);
            assert_eq!(flow_theme_cursor_speed(theme, f32::NAN), -1);
            assert_eq!(flow_theme_ansi(theme, [0u32; 15].as_ptr(), 15), -1);
            assert_eq!(flow_theme_ansi(theme, [0u32; 16].as_ptr(), 16), 0);
            assert_eq!(flow_theme_name(theme, std::ptr::null()), -1);
            assert_eq!(flow_theme_text(std::ptr::null_mut(), 0), -1);
            let lines = [c"a".as_ptr(), std::ptr::null()];
            assert_eq!(flow_theme_start(theme, lines.as_ptr(), 2), -1, "a NULL line");
            let mut error: *mut c_char = std::ptr::null_mut();
            assert_eq!(flow_theme_key(theme, c"fx".as_ptr(), c"liquid".as_ptr(), c"false".as_ptr(), &mut error), -1);
            assert_eq!(take(error).as_deref(), Some("“fx.liquid” isn't one of the look's settings a theme may bring"));
            assert_eq!(flow_theme_key(theme, c"fx".as_ptr(), c"sun".as_ptr(), c"bright".as_ptr(), &mut error), -1);
            assert!(take(error).unwrap().starts_with("fx.sun: "), "not JSON");
            assert_eq!(flow_theme_key(theme, c"fx".as_ptr(), c"sun".as_ptr(), c"\"bright\"".as_ptr(), &mut error), -1);
            assert_eq!(take(error).as_deref(), Some("“fx.sun” can't be \"bright\""));
            assert_eq!(flow_theme_check(theme, &mut error), 0);
            assert!(error.is_null());
            let bad = flow_theme_new(c"Mine".as_ptr());
            assert_eq!(flow_theme_check(bad, &mut error), -1);
            assert!(take(error).unwrap().contains("isn't an id a theme may have"));
            let package = flow_theme_package_new(c"mine".as_ptr(), c"1.0".as_ptr());
            assert_eq!(flow_theme_package_check(package, &mut error), -1);
            assert_eq!(take(error).as_deref(), Some("mine holds no themes"));
            assert_eq!(flow_theme_package_add(package, theme), 0);
            assert_eq!(flow_theme_package_check(package, &mut error), 0);
            assert_eq!(flow_theme_package_add(package, bad), 0);
            assert_eq!(take(flow_theme_package_json(package, &mut error)), None);
            assert!(!error.is_null());
            take(error);
            let out = std::env::temp_dir().join(format!("flow-sdk-c-theme-{}", std::process::id()));
            let out_c = std::ffi::CString::new(out.to_str().unwrap()).unwrap();
            assert_eq!(flow_theme_package_write(package, out_c.as_ptr(), &mut error), -1);
            assert!(!out.exists(), "nothing written");
            take(error);
            assert_eq!(flow_theme_package_check(std::ptr::null(), &mut error), -1);
            assert_eq!(take(error).as_deref(), Some("the package is NULL"));
            // A NULL error is let be.
            assert_eq!(flow_theme_package_check(std::ptr::null(), std::ptr::null_mut()), -1);
            // A file that can't be read fails otherwise than a refusal.
            let gone = flow_theme_new(c"gone".as_ptr());
            assert_eq!(flow_theme_font_file(gone, c"/nowhere/flow-sdk-gone.ttf".as_ptr()), 0, "looked for when checked");
            assert_eq!(flow_theme_check(gone, &mut error), FAILED);
            assert!(take(error).unwrap().starts_with("/nowhere/flow-sdk-gone.ttf: "));
            flow_theme_free(gone);
            flow_theme_free(theme);
            flow_theme_free(bad);
            flow_theme_package_free(package);
        }
    }
}
