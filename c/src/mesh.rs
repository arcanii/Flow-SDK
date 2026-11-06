//! Meshes from C ([`flow_sdk::mesh`]): an object built of flat convex faces
//! in its own space (y up), each facing away from a point inside it, made
//! into the GLB bytes that [`crate::model`] registers, for the terminal to
//! turn and light itself. A face is in a color, or in a `FlowMaterial` that
//! says how polished, metal, glowing, coated or unlit it is, as a scene's
//! polygons are.

use std::ffi::c_int;

use flow_sdk::mesh::Mesh;
use flow_sdk::scene::{Material, V3};

use crate::{given, guard, hand_over, slice};

/// An object being built: `flow_mesh_new` to `flow_mesh_free`.
pub struct FlowMesh(pub(crate) Mesh);

/// How a surface looks, as [`Material`] has it: [`flow_material_init`]
/// makes one a color alone, matte and lit.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowMaterial {
    /// Red, green, blue and alpha, 0 to 1 as shown.
    pub color: [f32; 4],
    /// 0 matte to 1 polished.
    pub gloss: f32,
    /// 0 to 1.
    pub metal: f32,
    /// The light it gives off, as shown; black for none.
    pub glow: [f32; 3],
    /// 1 as given.
    pub glow_strength: f32,
    /// A clear coat over it, 0 to 1, and how polished.
    pub coat: f32,
    pub coat_gloss: f32,
    /// Its color alone, no light falling on it.
    pub unlit: bool,
}

impl FlowMaterial {
    /// `color`, matte and lit.
    pub fn plain(color: [f32; 4]) -> Self {
        Self::from(Material::plain(color))
    }

    /// The SDK's material, if every number is.
    pub fn material(&self) -> Option<Material> {
        let rest = [self.gloss, self.metal, self.glow_strength, self.coat, self.coat_gloss];
        self.color.iter().chain(&self.glow).chain(&rest).all(|&v| given(v).is_some()).then_some(Material {
            color: self.color,
            gloss: self.gloss,
            metal: self.metal,
            glow: self.glow,
            glow_strength: self.glow_strength,
            coat: self.coat,
            coat_gloss: self.coat_gloss,
            unlit: self.unlit,
        })
    }
}

impl From<Material> for FlowMaterial {
    fn from(m: Material) -> Self {
        Self { color: m.color, gloss: m.gloss, metal: m.metal, glow: m.glow, glow_strength: m.glow_strength, coat: m.coat, coat_gloss: m.coat_gloss, unlit: m.unlit }
    }
}

/// The material at `ptr`, if its numbers are.
pub(crate) unsafe fn material(ptr: *const FlowMaterial) -> Option<Material> {
    ptr.as_ref()?.material()
}

/// Makes `material` the color `rgba` alone, matte and lit: set its other
/// fields after. -1 for NULL.
#[no_mangle]
pub unsafe extern "C" fn flow_material_init(material: *mut FlowMaterial, rgba: *const f32) -> c_int {
    guard(-1, || {
        let (Some(material), Some(rgba)) = (material.as_mut(), color(rgba)) else { return -1 };
        *material = FlowMaterial::plain(rgba);
        0
    })
}

/// Three floats: x, y and z.
pub(crate) unsafe fn point(ptr: *const f32) -> Option<V3> {
    slice(ptr, 3)?.try_into().ok()
}

/// Four floats: red, green, blue and alpha.
pub(crate) unsafe fn color(ptr: *const f32) -> Option<[f32; 4]> {
    slice(ptr, 4)?.try_into().ok()
}

/// `count` points, three floats each.
pub(crate) unsafe fn points(ptr: *const f32, count: usize) -> Option<Vec<V3>> {
    let floats = count.checked_mul(3).and_then(|n| slice(ptr, n))?;
    Some(floats.as_chunks::<3>().0.to_vec())
}

#[no_mangle]
pub extern "C" fn flow_mesh_new() -> *mut FlowMesh {
    guard(std::ptr::null_mut(), || Box::into_raw(Box::new(FlowMesh(Mesh::new()))))
}

#[no_mangle]
pub unsafe extern "C" fn flow_mesh_free(mesh: *mut FlowMesh) {
    if !mesh.is_null() {
        guard((), || drop(Box::from_raw(mesh)));
    }
}

/// A flat convex face of `count` points (`count` × 3 floats), in `color`
/// (red, green, blue and alpha, 0 to 1), facing away from `inside`.
#[no_mangle]
pub unsafe extern "C" fn flow_mesh_face(mesh: *mut FlowMesh, rgba: *const f32, points: *const f32, count: usize, inside: *const f32) -> c_int {
    guard(-1, || {
        let Some(mesh) = mesh.as_mut() else { return -1 };
        let (Some(rgba), Some(inside)) = (color(rgba), point(inside)) else { return -1 };
        let Some(points) = self::points(points, count).filter(|p| p.len() >= 3) else { return -1 };
        mesh.0.face(rgba, &points, inside);
        0
    })
}

/// [`flow_mesh_face`] in a material.
#[no_mangle]
pub unsafe extern "C" fn flow_mesh_face_in(mesh: *mut FlowMesh, look: *const FlowMaterial, points: *const f32, count: usize, inside: *const f32) -> c_int {
    guard(-1, || {
        let Some(mesh) = mesh.as_mut() else { return -1 };
        let (Some(look), Some(inside)) = (material(look), point(inside)) else { return -1 };
        let Some(points) = self::points(points, count).filter(|p| p.len() >= 3) else { return -1 };
        mesh.0.face(look, &points, inside);
        0
    })
}

#[no_mangle]
pub unsafe extern "C" fn flow_mesh_triangle(mesh: *mut FlowMesh, rgba: *const f32, a: *const f32, b: *const f32, c: *const f32, inside: *const f32) -> c_int {
    guard(-1, || {
        let Some(mesh) = mesh.as_mut() else { return -1 };
        let (Some(rgba), Some(a), Some(b), Some(c), Some(inside)) = (color(rgba), point(a), point(b), point(c), point(inside)) else { return -1 };
        mesh.0.triangle(rgba, a, b, c, inside);
        0
    })
}

#[no_mangle]
pub unsafe extern "C" fn flow_mesh_triangles(mesh: *const FlowMesh) -> usize {
    guard(0, || mesh.as_ref().map_or(0, |m| m.0.triangles()))
}

/// The object as a GLB, its length in `len`; NULL (and 0) for NULL.
#[no_mangle]
pub unsafe extern "C" fn flow_mesh_glb(mesh: *const FlowMesh, len: *mut usize) -> *mut u8 {
    guard(std::ptr::null_mut(), || {
        let (Some(mesh), Some(len)) = (mesh.as_ref(), len.as_mut()) else { return std::ptr::null_mut() };
        let glb = mesh.0.glb();
        let bytes = hand_over(&glb);
        *len = if bytes.is_null() { 0 } else { glb.len() };
        bytes
    })
}

/// [`flow_mesh_glb`] packed into whole numbers (KHR_mesh_quantization),
/// half the size, for Flow.
#[no_mangle]
pub unsafe extern "C" fn flow_mesh_glb_packed(mesh: *const FlowMesh, len: *mut usize) -> *mut u8 {
    guard(std::ptr::null_mut(), || {
        let (Some(mesh), Some(len)) = (mesh.as_ref(), len.as_mut()) else { return std::ptr::null_mut() };
        let glb = mesh.0.glb_packed();
        let bytes = hand_over(&glb);
        *len = if bytes.is_null() { 0 } else { glb.len() };
        bytes
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mesh_built_from_c_is_the_mesh_built_in_rust() {
        let gold = [0.9f32, 0.7, 0.2, 1.0];
        let square = [0.0f32, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 1.0, 1.0];
        let middle = [0.5f32, 0.5, 0.0];
        let mesh = flow_mesh_new();
        let mut len = 0usize;
        let mut rust = Mesh::new();
        rust.face(gold, &[[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0]], middle);
        rust.triangle(gold, [0.0, 0.0, -1.0], [1.0, 0.0, -1.0], [0.0, 1.0, -1.0], middle);
        unsafe {
            assert_eq!(flow_mesh_face(mesh, gold.as_ptr(), square.as_ptr(), 4, middle.as_ptr()), 0);
            assert_eq!(flow_mesh_triangle(mesh, gold.as_ptr(), [0.0f32, 0.0, -1.0].as_ptr(), [1.0f32, 0.0, -1.0].as_ptr(), [0.0f32, 1.0, -1.0].as_ptr(), middle.as_ptr()), 0);
            assert_eq!(flow_mesh_triangles(mesh), 3);
            let glb = flow_mesh_glb(mesh, &mut len);
            assert_eq!(std::slice::from_raw_parts(glb, len), &rust.glb()[..]);
            crate::flow_free(glb.cast());
            flow_mesh_free(mesh);
        }
    }

    #[test]
    fn a_face_in_a_material_from_c_is_the_rust_ones_packed_or_not() {
        let mut shiny = FlowMaterial { color: [0.1, 0.1, 0.12, 1.0], ..FlowMaterial::plain([1.0; 4]) };
        assert_eq!(shiny.material(), Some(Material::plain([0.1, 0.1, 0.12, 1.0])));
        shiny.gloss = 0.9;
        shiny.coat = 0.5;
        shiny.glow = [0.0, 0.5, 1.0];
        let square = [0.0f32, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 1.0, 1.0];
        let middle = [0.5f32, 0.5, 0.0];
        let mut rust = Mesh::new();
        rust.face(shiny.material().unwrap(), &[[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0]], middle);
        let mesh = flow_mesh_new();
        let mut init = FlowMaterial::plain([0.0; 4]);
        let mut len = 0usize;
        unsafe {
            assert_eq!(flow_material_init(&mut init, [0.2f32, 0.4, 0.6, 1.0].as_ptr()), 0);
            assert_eq!(init, FlowMaterial::plain([0.2, 0.4, 0.6, 1.0]));
            assert_eq!(flow_mesh_face_in(mesh, &shiny, square.as_ptr(), 4, middle.as_ptr()), 0);
            let glb = flow_mesh_glb(mesh, &mut len);
            assert_eq!(std::slice::from_raw_parts(glb, len), &rust.glb()[..]);
            crate::flow_free(glb.cast());
            let packed = flow_mesh_glb_packed(mesh, &mut len);
            assert_eq!(std::slice::from_raw_parts(packed, len), &rust.glb_packed()[..]);
            crate::flow_free(packed.cast());
            shiny.gloss = f32::NAN;
            assert_eq!(flow_mesh_face_in(mesh, &shiny, square.as_ptr(), 4, middle.as_ptr()), -1, "a number that isn't");
            assert_eq!(flow_mesh_face_in(mesh, std::ptr::null(), square.as_ptr(), 4, middle.as_ptr()), -1);
            assert_eq!(flow_material_init(std::ptr::null_mut(), [0.2f32, 0.4, 0.6, 1.0].as_ptr()), -1);
            assert!(flow_mesh_glb_packed(std::ptr::null(), &mut len).is_null());
            flow_mesh_free(mesh);
        }
    }

    #[test]
    fn a_face_without_enough_points_or_a_handle_is_refused() {
        let gold = [0.9f32, 0.7, 0.2, 1.0];
        let two = [0.0f32; 6];
        let mesh = flow_mesh_new();
        let mut len = 7usize;
        unsafe {
            assert_eq!(flow_mesh_face(mesh, gold.as_ptr(), two.as_ptr(), 2, [0.0f32; 3].as_ptr()), -1);
            assert_eq!(flow_mesh_face(mesh, gold.as_ptr(), std::ptr::null(), 3, [0.0f32; 3].as_ptr()), -1);
            assert_eq!(flow_mesh_face(mesh, std::ptr::null(), two.as_ptr(), 2, [0.0f32; 3].as_ptr()), -1);
            assert_eq!(flow_mesh_face(std::ptr::null_mut(), gold.as_ptr(), two.as_ptr(), 2, [0.0f32; 3].as_ptr()), -1);
            assert_eq!(flow_mesh_triangles(std::ptr::null()), 0);
            assert!(flow_mesh_glb(std::ptr::null(), &mut len).is_null());
            assert!(flow_mesh_glb(mesh, std::ptr::null_mut()).is_null());
            flow_mesh_free(mesh);
            flow_mesh_free(std::ptr::null_mut());
        }
    }
}
