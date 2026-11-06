//! 3D models from C, with the Ratty graphics protocol ([`flow_sdk::model`]):
//! an object registered by an id (a GLB or an OBJ, its bytes or a path),
//! placed centered on a cell over a block of cells, its look changed, and
//! deleted. How it looks is a plain struct, `FlowLook`, whose unset fields
//! (NAN, or -1) the terminal keeps as they were.

use std::ffi::c_char;

use flow_sdk::model::{self, Format, Look, Place};

use crate::{given, owned_or_null, slice, text};

/// `FLOW_MODEL_*`: a model's file format.
pub const MODEL_OBJ: i32 = 0;
pub const MODEL_GLB: i32 = 1;

/// How a placed object looks. [`flow_look_init`] leaves everything unset.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowLook {
    /// 1 spins, tumbles and bobs it; 0 holds it still; -1 leaves it.
    pub animate: i32,
    /// 0xRRGGBB tints an OBJ (a glTF has its own colors); -1 leaves it.
    pub color: i32,
    /// The floats are left as they were while NAN.
    pub scale: f32,
    /// Thickness for flat artwork, which also turns to an oblique view.
    pub depth: f32,
    pub brightness: f32,
    /// In pixels, x right, y up; z only moves it nearer or further.
    pub offset: [f32; 3],
    /// Degrees about x, y and z, turned in that order.
    pub rotation: [f32; 3],
    pub scale3: [f32; 3],
}

impl FlowLook {
    pub const UNSET: Self = Self { animate: -1, color: -1, scale: f32::NAN, depth: f32::NAN, brightness: f32::NAN, offset: [f32::NAN; 3], rotation: [f32::NAN; 3], scale3: [f32::NAN; 3] };

    /// The SDK's look: what's set, and nothing else.
    pub fn look(&self) -> Look {
        Look {
            animate: (self.animate >= 0).then_some(self.animate != 0),
            scale: given(self.scale),
            depth: given(self.depth),
            color: u32::try_from(self.color).ok(),
            brightness: given(self.brightness),
            offset: self.offset.map(given),
            rotation: self.rotation.map(given),
            scale3: self.scale3.map(given),
        }
    }
}

/// `look`, or none set for NULL.
unsafe fn look_at(look: *const FlowLook) -> Look {
    look.as_ref().map(FlowLook::look).unwrap_or_default()
}

fn format(format: i32) -> Option<Format> {
    match format {
        MODEL_OBJ => Some(Format::Obj),
        MODEL_GLB => Some(Format::Glb),
        _ => None,
    }
}

/// A name or a path as the escape can carry it: a `;` or a control
/// character would end it early.
unsafe fn field<'a>(ptr: *const c_char) -> Option<&'a str> {
    text(ptr).filter(|t| !t.chars().any(|c| c == ';' || c.is_control()))
}

#[no_mangle]
pub unsafe extern "C" fn flow_look_init(look: *mut FlowLook) {
    if let Some(look) = look.as_mut() {
        *look = FlowLook::UNSET;
    }
}

/// Registers a model's bytes in one command; `name` (or NULL) is the
/// file's name, for the terminal's messages.
#[no_mangle]
pub unsafe extern "C" fn flow_model_register(id: u32, fmt: i32, bytes: *const u8, len: usize, name: *const c_char) -> *mut c_char {
    flow_model_register_chunks(id, fmt, bytes, len, name, 0)
}

/// [`flow_model_register`] in commands of `chunk` bytes at most (0 for
/// one), each saying whether more follow.
#[no_mangle]
pub unsafe extern "C" fn flow_model_register_chunks(id: u32, fmt: i32, bytes: *const u8, len: usize, name: *const c_char, chunk: usize) -> *mut c_char {
    owned_or_null(|| {
        let name = if name.is_null() { None } else { Some(field(name)?) };
        let chunk = if chunk == 0 { usize::MAX } else { chunk };
        Some(model::register_chunks(id, format(fmt)?, slice(bytes, len)?, name, chunk))
    })
}

/// Registers the file at `path`, which Flow reads once the user allows it.
#[no_mangle]
pub unsafe extern "C" fn flow_model_register_path(id: u32, fmt: i32, path: *const c_char) -> *mut c_char {
    owned_or_null(|| Some(model::register_path(id, format(fmt)?, field(path).filter(|p| !p.is_empty())?)))
}

/// Places object `id` centered on cell (`row`, `col`), counting from 0,
/// over `cols` × `rows` cells.
#[no_mangle]
pub unsafe extern "C" fn flow_model_place(id: u32, row: u16, col: u16, cols: u16, rows: u16, look: *const FlowLook) -> *mut c_char {
    owned_or_null(|| Some(Place::new(id, (row, col), (cols, rows)).look(look_at(look)).escape()))
}

/// Places object `id` over the `cols` × `rows` cells whose top-left is
/// (`top`, `left`).
#[no_mangle]
pub unsafe extern "C" fn flow_model_place_over(id: u32, top: u16, left: u16, cols: u16, rows: u16, look: *const FlowLook) -> *mut c_char {
    let middle = |start: u16, span: u16| u32::from(start) + u32::from(span.saturating_sub(1).div_ceil(2)) <= u32::from(u16::MAX);
    owned_or_null(|| (middle(top, rows) && middle(left, cols)).then(|| Place::over(id, (top, left), (cols, rows)).look(look_at(look)).escape()))
}

#[no_mangle]
pub unsafe extern "C" fn flow_model_update(id: u32, look: *const FlowLook) -> *mut c_char {
    owned_or_null(|| Some(model::update(id, &look_at(look))))
}

#[no_mangle]
pub extern "C" fn flow_model_delete(id: u32) -> *mut c_char {
    owned_or_null(|| Some(model::delete(id)))
}

#[no_mangle]
pub extern "C" fn flow_model_delete_all() -> *mut c_char {
    owned_or_null(|| Some(model::DELETE_ALL.to_string()))
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
    fn an_unset_look_writes_nothing_and_a_set_one_its_fields() {
        let mut look = FlowLook { animate: 1, ..FlowLook::UNSET };
        unsafe { flow_look_init(&mut look) };
        assert_eq!(look.look(), Look::default());
        look.animate = 1;
        look.color = 0x7ce7ff;
        look.rotation[1] = 30.0;
        assert_eq!(taken(unsafe { flow_model_update(4, &look) }), "\x1b_ratty;g;u;id=4;animate=1;color=7ce7ff;ry=30\x1b\\");
        assert_eq!(taken(unsafe { flow_model_update(4, std::ptr::null()) }), "\x1b_ratty;g;u;id=4\x1b\\");
    }

    #[test]
    fn a_model_is_registered_placed_and_deleted_as_the_sdk_does() {
        let reg = taken(unsafe { flow_model_register(1, MODEL_GLB, b"abc".as_ptr(), 3, c"scene.glb".as_ptr()) });
        assert_eq!(reg, model::register(1, Format::Glb, b"abc", Some("scene.glb")));
        let chunks = taken(unsafe { flow_model_register_chunks(3, MODEL_OBJ, b"abcdefg".as_ptr(), 7, std::ptr::null(), 3) });
        assert_eq!(chunks, model::register_chunks(3, Format::Obj, b"abcdefg", None, 3));
        assert_eq!(taken(unsafe { flow_model_register_path(2, MODEL_OBJ, c"rat.obj".as_ptr()) }), model::register_path(2, Format::Obj, "rat.obj"));
        let look = FlowLook { scale: model::FILL, ..FlowLook::UNSET };
        assert_eq!(taken(unsafe { flow_model_place_over(1, 2, 4, 80, 23, &look) }), "\x1b_ratty;g;p;id=1;row=13;col=44;w=80;h=23;scale=1.1111112\x1b\\");
        assert_eq!(taken(unsafe { flow_model_place(1, 13, 44, 80, 23, std::ptr::null()) }), "\x1b_ratty;g;p;id=1;row=13;col=44;w=80;h=23\x1b\\");
        assert_eq!(taken(flow_model_delete(4)), model::delete(4));
        assert_eq!(taken(flow_model_delete_all()), model::DELETE_ALL);
    }

    #[test]
    fn what_would_break_the_escape_is_refused() {
        unsafe {
            assert!(flow_model_register(1, 9, b"abc".as_ptr(), 3, std::ptr::null()).is_null(), "no such format");
            assert!(flow_model_register(1, MODEL_GLB, std::ptr::null(), 3, std::ptr::null()).is_null());
            assert!(flow_model_register(1, MODEL_GLB, b"abc".as_ptr(), 3, c"a;b.glb".as_ptr()).is_null());
            assert!(flow_model_register_path(1, MODEL_OBJ, c"x\x1b\\.obj".as_ptr()).is_null());
            assert!(flow_model_register_path(1, MODEL_OBJ, std::ptr::null()).is_null());
            assert!(flow_model_place_over(1, u16::MAX, 0, 1, 4, std::ptr::null()).is_null());
        }
    }
}
