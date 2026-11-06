//! Flow's liquid and its sounds from C ([`flow_sdk::fx`]): an effect at a
//! cell built on a `FlowEffect` handle and written as its OSC 7731 escape,
//! one of Flow's sounds by name, DECPS notes, and the glass's tint; and a
//! `FlowBudget` that keeps a program under Flow's allowance of them.

use std::ffi::{c_char, c_int};

use flow_sdk::fx::{self, Budget, Effect, Kind, Sound};

use crate::{given, guard, owned_or_null, slice, text};

/// `FLOW_FX_*`: what an effect does.
pub const FX_BOLT: i32 = 0;
pub const FX_BURST: i32 = 1;
pub const FX_RIPPLE: i32 = 2;
pub const FX_STIR: i32 = 3;

/// An effect being built: `flow_effect_new` to `flow_effect_free`.
pub struct FlowEffect(pub(crate) Effect);

pub(crate) fn kind(kind: i32) -> Option<Kind> {
    match kind {
        FX_BOLT => Some(Kind::Bolt),
        FX_BURST => Some(Kind::Burst),
        FX_RIPPLE => Some(Kind::Ripple),
        FX_STIR => Some(Kind::Stir),
        _ => None,
    }
}

/// `FLOW_SOUND_*` is the place in [`Sound::ALL`]: drop 0 to bell 10.
fn sound(n: i32) -> Option<Sound> {
    Sound::ALL.get(usize::try_from(n).ok()?).copied()
}

/// A budget for effects and named sounds: `flow_budget_new` to
/// `flow_budget_free`.
pub struct FlowBudget(pub(crate) Budget);

/// A full budget of `burst` coming back at `per_second` a second (NAN for
/// either takes the SDK's: 7, then 5 a second, a little under Flow's 8 and
/// 6).
#[no_mangle]
pub extern "C" fn flow_budget_new(per_second: f32, burst: f32) -> *mut FlowBudget {
    guard(std::ptr::null_mut(), || Box::into_raw(Box::new(FlowBudget(Budget::with(given(per_second).unwrap_or(Budget::PER_SECOND), given(burst).unwrap_or(Budget::BURST))))))
}

#[no_mangle]
pub unsafe extern "C" fn flow_budget_free(budget: *mut FlowBudget) {
    if !budget.is_null() {
        guard((), || drop(Box::from_raw(budget)));
    }
}

/// Spends one at `now` (the program's own clock, in seconds), if `keep`
/// would still be left after it; false, and nothing spent, if not or for
/// NULL.
#[no_mangle]
pub unsafe extern "C" fn flow_budget_take(budget: *mut FlowBudget, now: f32, keep: f32) -> bool {
    guard(false, || match (budget.as_mut(), given(now), given(keep)) {
        (Some(budget), Some(now), Some(keep)) => budget.0.take(now, keep),
        _ => false,
    })
}

/// How many are left at `now`; NAN for NULL.
#[no_mangle]
pub unsafe extern "C" fn flow_budget_left(budget: *const FlowBudget, now: f32) -> f32 {
    guard(f32::NAN, || match (budget.as_ref(), given(now)) {
        (Some(budget), Some(now)) => budget.0.left(now),
        _ => f32::NAN,
    })
}

/// An effect of `kind` at cell (`col`, `row`), counting from the screen's
/// top-left from 0; NULL for a kind there's none of. A bolt needs
/// [`flow_effect_to`] and a stir [`flow_effect_dir`], or Flow ignores it.
#[no_mangle]
pub extern "C" fn flow_effect_new(fx_kind: i32, col: u16, row: u16) -> *mut FlowEffect {
    guard(std::ptr::null_mut(), || match kind(fx_kind) {
        Some(k) => Box::into_raw(Box::new(FlowEffect(Effect::new(k, (col, row))))),
        None => std::ptr::null_mut(),
    })
}

#[no_mangle]
pub unsafe extern "C" fn flow_effect_free(effect: *mut FlowEffect) {
    if !effect.is_null() {
        guard((), || drop(Box::from_raw(effect)));
    }
}

/// Changes the effect behind `effect` by `change`: 0, or -1 for NULL or a
/// value refused.
unsafe fn with(effect: *mut FlowEffect, change: impl FnOnce(Effect) -> Option<Effect>) -> c_int {
    guard(-1, || {
        let Some(effect) = effect.as_mut() else { return -1 };
        match change(effect.0.clone()) {
            Some(changed) => {
                effect.0 = changed;
                0
            }
            None => -1,
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn flow_effect_to(effect: *mut FlowEffect, col: u16, row: u16) -> c_int {
    with(effect, |e| Some(e.to((col, row))))
}

#[no_mangle]
pub unsafe extern "C" fn flow_effect_color(effect: *mut FlowEffect, rgb: u32) -> c_int {
    with(effect, |e| Some(e.color(rgb & 0xff_ffff)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_effect_dir(effect: *mut FlowEffect, dx: f32, dy: f32) -> c_int {
    with(effect, |e| Some(e.dir((given(dx)?, given(dy)?))))
}

#[no_mangle]
pub unsafe extern "C" fn flow_effect_size(effect: *mut FlowEffect, columns: f32) -> c_int {
    with(effect, |e| Some(e.size(given(columns)?)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_effect_strength(effect: *mut FlowEffect, strength: f32) -> c_int {
    with(effect, |e| Some(e.strength(given(strength)?)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_effect_volume(effect: *mut FlowEffect, volume: f32) -> c_int {
    with(effect, |e| Some(e.volume(given(volume)?)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_effect_sound(effect: *mut FlowEffect, fx_sound: i32) -> c_int {
    with(effect, |e| Some(e.sound(sound(fx_sound)?)))
}

#[no_mangle]
pub unsafe extern "C" fn flow_effect_quiet(effect: *mut FlowEffect) -> c_int {
    with(effect, |e| Some(e.quiet()))
}

#[no_mangle]
pub unsafe extern "C" fn flow_effect_escape(effect: *const FlowEffect) -> *mut c_char {
    owned_or_null(|| Some(effect.as_ref()?.0.escape()))
}

/// One of Flow's sounds alone: placed left or right by `col` (none if
/// `col` or `row` is negative), as strong as `strength` (NAN for Flow's
/// 0.5), at `volume` (NAN or 1 for full).
#[no_mangle]
pub extern "C" fn flow_sound(fx_sound: i32, col: i32, row: i32, strength: f32, volume: f32) -> *mut c_char {
    owned_or_null(|| {
        let at = u16::try_from(col).ok().zip(u16::try_from(row).ok());
        Some(sound(fx_sound)?.escape_with(at, given(strength), given(volume).unwrap_or(1.0)))
    })
}

/// The `FLOW_SOUND_*` number for a sound's name (any case), or -1.
#[no_mangle]
pub unsafe extern "C" fn flow_sound_named(name: *const c_char) -> c_int {
    guard(-1, || {
        let Some(found) = text(name).and_then(Sound::named) else { return -1 };
        Sound::ALL.iter().position(|s| *s == found).map_or(-1, |i| i as c_int)
    })
}

/// DECPS notes: `volume` 0 to 7, each note `length` 32nds of a second, and
/// `count` notes from 1 (C5) to 25 (C7), 0 a rest. NULL for no notes.
#[no_mangle]
pub unsafe extern "C" fn flow_notes(volume: u8, length: u8, notes: *const u8, count: usize) -> *mut c_char {
    owned_or_null(|| Some(fx::notes(volume, length, slice(notes, count)?)).filter(|e| !e.is_empty()))
}

/// DECPS's number for a note by name (`C5` 1 to `C7` 25, `C#5` or `Db5`,
/// `r` a rest 0), or -1.
#[no_mangle]
pub unsafe extern "C" fn flow_note(name: *const c_char) -> c_int {
    guard(-1, || text(name).and_then(fx::note).map_or(-1, c_int::from))
}

/// The glass washed in `rgb` (0xRRGGBB) as far as `strength` (0 to 1)
/// says while the program holds the alternate screen; 0 washes it off.
#[no_mangle]
pub extern "C" fn flow_tint(rgb: u32, strength: f32) -> *mut c_char {
    owned_or_null(|| Some(fx::tint(rgb, strength)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    fn taken(s: *mut c_char) -> String {
        assert!(!s.is_null());
        // SAFETY: handed out by this crate, freed once.
        unsafe {
            let text = CStr::from_ptr(s).to_str().unwrap().to_string();
            crate::flow_free(s.cast());
            text
        }
    }

    #[test]
    fn an_effect_built_from_c_writes_what_the_sdk_writes() {
        let bolt = flow_effect_new(FX_BOLT, 3, 4);
        let stir = flow_effect_new(FX_STIR, 4, 7);
        unsafe {
            assert_eq!(flow_effect_to(bolt, 10, 4) | flow_effect_color(bolt, 0xff5a1a) | flow_effect_size(bolt, 2.0) | flow_effect_strength(bolt, 0.8), 0);
            assert_eq!(taken(flow_effect_escape(bolt)), Effect::bolt((3, 4), (10, 4)).color(0xff5a1a).size(2.0).strength(0.8).escape());
            assert_eq!(flow_effect_dir(stir, 1.0, -1.0) | flow_effect_sound(stir, 7) | flow_effect_volume(stir, 0.4), 0);
            assert_eq!(taken(flow_effect_escape(stir)), Effect::stir((4, 7), (1.0, -1.0)).sound(Sound::Thud).volume(0.4).escape());
            assert_eq!(flow_effect_quiet(stir), 0);
            assert!(taken(flow_effect_escape(stir)).contains(";sound=none;"));
            flow_effect_free(bolt);
            flow_effect_free(stir);
        }
    }

    #[test]
    fn a_budget_from_c_spends_as_the_sdks_does() {
        let budget = flow_budget_new(f32::NAN, f32::NAN);
        let own = flow_budget_new(1.0, 2.0);
        unsafe {
            assert_eq!(flow_budget_left(budget, 0.0), Budget::BURST);
            assert_eq!((0..9).filter(|_| flow_budget_take(budget, 0.0, 2.0)).count(), 5, "two kept back");
            assert!(flow_budget_take(own, 1.0, 0.0) && flow_budget_take(own, 1.0, 0.0) && !flow_budget_take(own, 1.5, 0.0));
            assert!((flow_budget_left(own, 1.5) - 0.5).abs() < 1e-6);
            assert!(!flow_budget_take(std::ptr::null_mut(), 0.0, 0.0) && !flow_budget_take(own, f32::NAN, 0.0));
            assert!(flow_budget_left(std::ptr::null(), 0.0).is_nan());
            flow_budget_free(budget);
            flow_budget_free(own);
            flow_budget_free(std::ptr::null_mut());
        }
    }

    #[test]
    fn values_out_of_range_are_refused_and_the_effect_kept() {
        assert!(flow_effect_new(9, 0, 0).is_null());
        let burst = flow_effect_new(FX_BURST, 1, 1);
        unsafe {
            assert_eq!(flow_effect_sound(burst, 11), -1);
            assert_eq!(flow_effect_sound(burst, -1), -1);
            assert_eq!(flow_effect_size(burst, f32::NAN), -1);
            assert_eq!(flow_effect_dir(burst, 1.0, f32::INFINITY), -1);
            assert_eq!(taken(flow_effect_escape(burst)), Effect::burst((1, 1)).escape());
            assert_eq!(flow_effect_color(std::ptr::null_mut(), 0), -1);
            assert!(flow_effect_escape(std::ptr::null()).is_null());
            flow_effect_free(burst);
            flow_effect_free(std::ptr::null_mut());
        }
    }

    #[test]
    fn sounds_notes_and_the_tint_go_as_the_sdk_writes_them() {
        assert_eq!(taken(flow_sound(4, 30, 4, 0.8, f32::NAN)), Sound::Chime.escape(Some((30, 4)), Some(0.8)));
        assert_eq!(taken(flow_sound(8, -1, 0, f32::NAN, 0.5)), Sound::Click.escape_with(None, None, 0.5));
        assert!(flow_sound(11, 0, 0, 0.5, 1.0).is_null());
        unsafe {
            assert_eq!(flow_sound_named(c"BOING".as_ptr()), 9);
            assert_eq!(flow_sound_named(c"kazoo".as_ptr()), -1);
            assert_eq!(flow_sound_named(std::ptr::null()), -1);
            assert_eq!((flow_note(c"C#5".as_ptr()), flow_note(c"r".as_ptr()), flow_note(c"H5".as_ptr())), (2, 0, -1));
            assert_eq!(taken(flow_notes(9, 8, [1u8, 5, 8].as_ptr(), 3)), fx::notes(9, 8, &[1, 5, 8]));
            assert!(flow_notes(3, 8, std::ptr::null(), 0).is_null(), "DECPS needs a note");
        }
        assert_eq!(taken(flow_tint(0x0c1838, 0.55)), fx::tint(0x0c1838, 0.55));
        assert_eq!(taken(flow_tint(0, 0.0)), fx::UNTINT);
        assert!(Sound::ALL.iter().enumerate().all(|(i, s)| sound(i as i32) == Some(*s)));
    }
}
