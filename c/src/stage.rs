//! Stages from C ([`flow_sdk::stage`]): where a scene goes over the
//! terminal's cells (as many as Flow lets one placement cover, centered
//! above the rows the program keeps for its words), each frame's scene
//! sent as the same object, placed the first time and not sent at all when
//! it's the last frame again, and points of the picture and the screen's
//! cells mapped both ways, for clicks and for the liquid's effects.

use std::ffi::{c_char, c_int};

use flow_sdk::stage::{Fx, Stage};
use flow_sdk::term::Size;

use crate::fx::{self, FlowEffect};
use crate::scene::FlowScene;
use crate::term::FlowSize;
use crate::{given, guard, owned, owned_or_null, slice, text};

/// A stage: `flow_stage_new` to `flow_stage_free`.
pub struct FlowStage(pub(crate) Stage);

/// Where a stage is on the screen, in cells counted from 0, and how big
/// its picture is in the screen's pixels: a scene for it is that size.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FlowStageLayout {
    pub left: u16,
    pub top: u16,
    pub cols: u16,
    pub rows: u16,
    /// The first row below the picture, where the program's words go.
    pub below: u16,
    pub width: f32,
    pub height: f32,
}

/// Two floats, finite: a point or a direction.
unsafe fn pair(ptr: *const f32) -> Option<[f32; 2]> {
    let [x, y]: [f32; 2] = slice(ptr, 2)?.try_into().ok()?;
    (x.is_finite() && y.is_finite()).then_some([x, y])
}

/// The stage for a terminal `size` (its cells, and a cell's pixels on the
/// screen where known, else 0) whose cells have `cell_width` ×
/// `cell_height` whole pixels (`FlowCaps`' cell, 0 if not said), keeping
/// `words` rows below it. NULL if it would be under 16 × 6 cells, or no
/// cell size is known at all.
#[no_mangle]
pub unsafe extern "C" fn flow_stage_new(size: *const FlowSize, cell_width: u16, cell_height: u16, words: u16) -> *mut FlowStage {
    guard(std::ptr::null_mut(), || {
        let Some(size) = size.as_ref() else { return std::ptr::null_mut() };
        let known = |v: f32| v.is_finite() && v > 0.0;
        let cell = (known(size.cell_width) && known(size.cell_height)).then_some((size.cell_width, size.cell_height));
        let whole = (cell_width > 0 && cell_height > 0).then_some((cell_width, cell_height));
        match Stage::new(Size { cols: size.cols, rows: size.rows, cell }, whole, words) {
            Some(stage) => Box::into_raw(Box::new(FlowStage(stage))),
            None => std::ptr::null_mut(),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn flow_stage_free(stage: *mut FlowStage) {
    if !stage.is_null() {
        guard((), || drop(Box::from_raw(stage)));
    }
}

/// Its object's id, in place of 1.
#[no_mangle]
pub unsafe extern "C" fn flow_stage_id(stage: *mut FlowStage, id: u32) -> c_int {
    guard(-1, || {
        let Some(stage) = stage.as_mut() else { return -1 };
        stage.0 = stage.0.clone().id(id);
        0
    })
}

/// Sends each frame packed into whole numbers (KHR_mesh_quantization),
/// half the size, for Flow (`packed` true), or as floats (false, as made).
#[no_mangle]
pub unsafe extern "C" fn flow_stage_packed(stage: *mut FlowStage, packed: bool) -> c_int {
    guard(-1, || {
        let Some(stage) = stage.as_mut() else { return -1 };
        stage.0 = stage.0.clone().packed(packed);
        0
    })
}

/// Names the program in each frame's GLB, in place of `flow-sdk`.
#[no_mangle]
pub unsafe extern "C" fn flow_stage_made_by(stage: *mut FlowStage, generator: *const c_char) -> c_int {
    guard(-1, || {
        let (Some(stage), Some(generator)) = (stage.as_mut(), text(generator)) else { return -1 };
        stage.0 = stage.0.clone().made_by(generator.to_string());
        0
    })
}

#[no_mangle]
pub unsafe extern "C" fn flow_stage_layout(stage: *const FlowStage, out: *mut FlowStageLayout) -> c_int {
    guard(-1, || {
        let (Some(stage), Some(out)) = (stage.as_ref(), out.as_mut()) else { return -1 };
        let s = &stage.0;
        *out = FlowStageLayout { left: s.left, top: s.top, cols: s.cols, rows: s.rows, below: s.below(), width: s.width(), height: s.height() };
        0
    })
}

/// The screen cell a point of the picture (`x`, `y`, pixels from its
/// middle, y up) is in, or the nearest on the picture.
#[no_mangle]
pub unsafe extern "C" fn flow_stage_cell_at(stage: *const FlowStage, x: f32, y: f32, col: *mut u16, row: *mut u16) -> c_int {
    guard(-1, || {
        let (Some(stage), Some(col), Some(row)) = (stage.as_ref(), col.as_mut(), row.as_mut()) else { return -1 };
        if !(x.is_finite() && y.is_finite()) {
            return -1;
        }
        (*col, *row) = stage.0.cell_at([x, y]);
        0
    })
}

/// The point of the picture at the middle of screen cell (`col`, `row`);
/// -1 if the cell isn't on the picture.
#[no_mangle]
pub unsafe extern "C" fn flow_stage_point_at(stage: *const FlowStage, col: u16, row: u16, x: *mut f32, y: *mut f32) -> c_int {
    guard(-1, || {
        let (Some(stage), Some(x), Some(y)) = (stage.as_ref(), x.as_mut(), y.as_mut()) else { return -1 };
        let Some([px, py]) = stage.0.point_at(col, row) else { return -1 };
        (*x, *y) = (px, py);
        0
    })
}

/// What shows `scene` (made `width` × `height` from the layout): the
/// object registered again, and the first time where it goes. "" when
/// it's the last frame again, which needn't be sent.
#[no_mangle]
pub unsafe extern "C" fn flow_stage_show(stage: *mut FlowStage, scene: *const FlowScene) -> *mut c_char {
    guard(std::ptr::null_mut(), || {
        let (Some(stage), Some(scene)) = (stage.as_mut(), scene.as_ref()) else { return std::ptr::null_mut() };
        let mut out = Vec::new();
        stage.0.show(&scene.0, &mut out);
        match String::from_utf8(out) {
            Ok(commands) => owned(commands),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Takes the picture away.
#[no_mangle]
pub unsafe extern "C" fn flow_stage_clear(stage: *const FlowStage) -> *mut c_char {
    owned_or_null(|| {
        let mut out = Vec::new();
        stage.as_ref()?.0.clear(&mut out);
        String::from_utf8(out).ok()
    })
}

/// An effect for Flow's liquid at point `at` of the picture, reaching
/// `size` pixels as hard as `strength`, put in the screen's cells; `to` is
/// where a bolt ends and `dir` which way a stir pushes (y up), each NULL
/// for none. NULL for a kind there's none of or a number that isn't
/// finite.
#[no_mangle]
pub unsafe extern "C" fn flow_stage_effect(stage: *const FlowStage, fx_kind: i32, at: *const f32, to: *const f32, dir: *const f32, size: f32, strength: f32) -> *mut FlowEffect {
    let made = || -> Option<FlowEffect> {
        let stage = stage.as_ref()?;
        let mut effect = Fx::new(fx::kind(fx_kind)?, pair(at)?, given(size)?, given(strength)?);
        if !to.is_null() {
            effect.to = Some(pair(to)?);
        }
        if !dir.is_null() {
            effect.dir = Some(pair(dir)?);
        }
        Some(FlowEffect(stage.0.effect(&effect)))
    };
    guard(std::ptr::null_mut(), || made().map_or(std::ptr::null_mut(), |e| Box::into_raw(Box::new(e))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::{flow_effect_color, flow_effect_escape, flow_effect_free, flow_effect_sound, FX_BOLT, FX_STIR};
    use crate::scene::{flow_scene_flat, flow_scene_free, flow_scene_new};
    use flow_sdk::fx::{Kind, Sound};
    use flow_sdk::scene::{rgb, Scene};
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

    fn size() -> FlowSize {
        FlowSize { cols: 80, rows: 24, cell_width: 9.5, cell_height: 19.0 }
    }

    #[test]
    fn a_stage_from_c_lays_out_and_sends_what_the_rust_stage_does() {
        let mut rust = Stage::new(Size { cols: 80, rows: 24, cell: Some((9.5, 19.0)) }, Some((9, 19)), 1).expect("big enough").made_by("a-test");
        let stage = unsafe { flow_stage_new(&size(), 9, 19, 1) };
        let mut layout = FlowStageLayout::default();
        unsafe {
            assert_eq!(flow_stage_made_by(stage, c"a-test".as_ptr()), 0);
            assert_eq!(flow_stage_layout(stage, &mut layout), 0);
        }
        assert_eq!(layout, FlowStageLayout { left: rust.left, top: rust.top, cols: rust.cols, rows: rust.rows, below: rust.below(), width: rust.width(), height: rust.height() });

        let triangle = [[0.0f32, 0.0, 0.0], [50.0, 0.0, 0.0], [0.0, 50.0, 0.0]];
        let mut theirs = Scene::new(rust.width(), rust.height());
        theirs.flat(rgb(0xff0000), &triangle, [0.0, 0.0, 1.0]);
        let scene = flow_scene_new(layout.width, layout.height);
        let flat: Vec<f32> = triangle.iter().flatten().copied().collect();
        unsafe { assert_eq!(flow_scene_flat(scene, rgb(0xff0000).as_ptr(), flat.as_ptr(), 3, [0.0f32, 0.0, 1.0].as_ptr()), 0) };
        let mut first = Vec::new();
        rust.show(&theirs, &mut first);
        assert_eq!(taken(unsafe { flow_stage_show(stage, scene) }), String::from_utf8(first).unwrap(), "registered and placed");
        assert_eq!(taken(unsafe { flow_stage_show(stage, scene) }), "", "the same frame isn't sent again");
        // Packed, the same scene is a new frame, sent as the Rust stage
        // sends it packed.
        let mut packed = Vec::new();
        rust = rust.packed(true);
        rust.show(&theirs, &mut packed);
        unsafe { assert_eq!(flow_stage_packed(stage, true), 0) };
        assert_eq!(taken(unsafe { flow_stage_show(stage, scene) }), String::from_utf8(packed).unwrap());
        unsafe {
            assert_eq!(flow_stage_packed(std::ptr::null_mut(), true), -1);
            assert_eq!(flow_stage_id(stage, 7), 0);
            assert_eq!(taken(flow_stage_clear(stage)), "\x1b_ratty;g;d;id=7\x1b\\");
            flow_scene_free(scene);
            flow_stage_free(stage);
        }
    }

    #[test]
    fn clicks_and_effects_land_in_the_cells_the_rust_stage_puts_them() {
        let rust = Stage::new(Size { cols: 80, rows: 24, cell: Some((9.5, 19.0)) }, Some((9, 19)), 1).expect("big enough");
        let stage = unsafe { flow_stage_new(&size(), 9, 19, 1) };
        let (mut x, mut y, mut col, mut row) = (0.0f32, 0.0f32, 0u16, 0u16);
        unsafe {
            assert_eq!(flow_stage_point_at(stage, 30, 10, &mut x, &mut y), 0);
            assert_eq!(Some([x, y]), rust.point_at(30, 10));
            assert_eq!(flow_stage_cell_at(stage, x, y, &mut col, &mut row), 0);
            assert_eq!((col, row), (30, 10));
            assert_eq!(flow_stage_point_at(stage, 30, 23, &mut x, &mut y), -1, "the words' row isn't the picture");
        }
        let at = rust.point_at(30, 10).unwrap();
        let stir = Fx::new(Kind::Stir, at, 90.0, 0.5).color(0xe02020).dir([1.0, 1.0]).sound(Sound::Thud);
        let bolt = Fx::new(Kind::Bolt, at, 30.0, 0.8).to([0.0, 0.0]);
        unsafe {
            let e = flow_stage_effect(stage, FX_STIR, at.as_ptr(), std::ptr::null(), [1.0f32, 1.0].as_ptr(), 90.0, 0.5);
            assert_eq!(flow_effect_color(e, 0xe02020) | flow_effect_sound(e, Sound::ALL.iter().position(|s| *s == Sound::Thud).unwrap() as i32), 0);
            assert_eq!(taken(flow_effect_escape(e)), rust.effect(&stir).escape(), "its color and sound set on the effect after");
            flow_effect_free(e);
            let e = flow_stage_effect(stage, FX_BOLT, at.as_ptr(), [0.0f32, 0.0].as_ptr(), std::ptr::null(), 30.0, 0.8);
            assert_eq!(taken(flow_effect_escape(e)), rust.effect(&bolt).escape());
            flow_effect_free(e);
            flow_stage_free(stage);
        }
    }

    #[test]
    fn a_stage_too_small_or_given_nothing_is_refused() {
        let small = FlowSize { cols: 12, rows: 5, cell_width: 9.0, cell_height: 18.0 };
        let unknown = FlowSize { cell_width: 0.0, cell_height: 0.0, ..size() };
        let mut layout = FlowStageLayout::default();
        let (mut col, mut row) = (0u16, 0u16);
        unsafe {
            assert!(flow_stage_new(&small, 9, 18, 1).is_null());
            assert!(flow_stage_new(&unknown, 0, 0, 1).is_null(), "no cell size known at all");
            assert!(flow_stage_new(std::ptr::null(), 9, 18, 1).is_null());
            let stage = flow_stage_new(&unknown, 9, 19, 1);
            assert!(!stage.is_null(), "the whole pixels will do");
            assert_eq!(flow_stage_layout(std::ptr::null(), &mut layout), -1);
            assert_eq!(flow_stage_cell_at(stage, f32::NAN, 0.0, &mut col, &mut row), -1);
            assert_eq!(flow_stage_made_by(stage, std::ptr::null()), -1);
            assert!(flow_stage_show(stage, std::ptr::null()).is_null());
            assert!(flow_stage_clear(std::ptr::null()).is_null());
            assert!(flow_stage_effect(stage, 9, [0.0f32; 2].as_ptr(), std::ptr::null(), std::ptr::null(), 10.0, 0.5).is_null(), "no such kind");
            assert!(flow_stage_effect(stage, FX_BOLT, [0.0f32; 2].as_ptr(), [f32::NAN, 0.0].as_ptr(), std::ptr::null(), 10.0, 0.5).is_null());
            assert!(flow_stage_effect(stage, FX_BOLT, std::ptr::null(), std::ptr::null(), std::ptr::null(), 10.0, 0.5).is_null());
            flow_stage_free(stage);
            flow_stage_free(std::ptr::null_mut());
        }
    }
}
