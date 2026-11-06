//! Scenes from C ([`flow_sdk::scene`]): a frame's picture built of flat
//! convex polygons in its own pixels (from its middle, y up, z only
//! ordering what's in front), each in a color and cut to the picture, made
//! into the GLB bytes the terminal draws; a `FlowCamera` that turns a world
//! in 3D into those pixels, in perspective; and the color to give a face
//! for it to show as wanted once Flow's light falls on it. A polygon is in
//! a color, or in a `FlowMaterial` ([`crate::mesh`]). A stage
//! ([`crate::stage`]) sends a scene each frame and fits it to the cells.
//! And the vector maths to work a scene out with: points added, scaled and
//! crossed, rotations made and composed, colors mixed.

use std::ffi::{c_char, c_int};

use flow_sdk::scene::{self, Camera, Corner, Scene, M3};

use crate::mesh::{color, material, point, points, FlowMaterial};
use crate::{given, guard, hand_over, slice, text};

/// A frame's picture being built: `flow_scene_new` to `flow_scene_free`.
pub struct FlowScene(pub(crate) Scene);

/// A camera aimed at a world: `flow_camera_orbit` to `flow_camera_free`.
pub struct FlowCamera(pub(crate) Camera);

/// `count` corners, six floats each: where the corner is (x, y, z) and
/// which way the surface faces there.
unsafe fn corners(ptr: *const f32, count: usize) -> Option<Vec<Corner>> {
    let floats = count.checked_mul(6).and_then(|n| slice(ptr, n))?;
    Some(floats.as_chunks::<6>().0.iter().map(|c| Corner { at: [c[0], c[1], c[2]], normal: [c[3], c[4], c[5]] }).collect())
}

/// Writes `values` where `out` points, if it points anywhere: through the
/// pointer itself, so `out` may be one of the arguments just read.
unsafe fn put<const N: usize>(out: *mut f32, values: [f32; N]) -> bool {
    if out.is_null() {
        return false;
    }
    for (i, v) in values.into_iter().enumerate() {
        out.add(i).write(v);
    }
    true
}

/// A picture `width` × `height` pixels as the screen shows them; NULL for
/// sides that aren't above 0.
#[no_mangle]
pub extern "C" fn flow_scene_new(width: f32, height: f32) -> *mut FlowScene {
    guard(std::ptr::null_mut(), || match (given(width), given(height)) {
        (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Box::into_raw(Box::new(FlowScene(Scene::new(w, h)))),
        _ => std::ptr::null_mut(),
    })
}

#[no_mangle]
pub unsafe extern "C" fn flow_scene_free(scene: *mut FlowScene) {
    if !scene.is_null() {
        guard((), || drop(Box::from_raw(scene)));
    }
}

/// A flat convex polygon of `count` corners, cut to the picture and wound
/// as Flow lights it, whichever way its corners go round.
#[no_mangle]
pub unsafe extern "C" fn flow_scene_polygon(scene: *mut FlowScene, rgba: *const f32, corners: *const f32, count: usize) -> c_int {
    guard(-1, || {
        let (Some(scene), Some(rgba)) = (scene.as_mut(), color(rgba)) else { return -1 };
        let Some(corners) = self::corners(corners, count).filter(|c| c.len() >= 3) else { return -1 };
        scene.0.polygon(rgba, &corners);
        0
    })
}

/// [`flow_scene_polygon`] in a material.
#[no_mangle]
pub unsafe extern "C" fn flow_scene_polygon_in(scene: *mut FlowScene, look: *const FlowMaterial, corners: *const f32, count: usize) -> c_int {
    guard(-1, || {
        let (Some(scene), Some(look)) = (scene.as_mut(), material(look)) else { return -1 };
        let Some(corners) = self::corners(corners, count).filter(|c| c.len() >= 3) else { return -1 };
        scene.0.polygon(look, &corners);
        0
    })
}

/// A polygon of `count` points facing `normal` all over.
#[no_mangle]
pub unsafe extern "C" fn flow_scene_flat(scene: *mut FlowScene, rgba: *const f32, points: *const f32, count: usize, normal: *const f32) -> c_int {
    guard(-1, || {
        let (Some(scene), Some(rgba), Some(normal)) = (scene.as_mut(), color(rgba), point(normal)) else { return -1 };
        let Some(points) = self::points(points, count).filter(|p| p.len() >= 3) else { return -1 };
        scene.0.flat(rgba, &points, normal);
        0
    })
}

/// [`flow_scene_flat`] in a material.
#[no_mangle]
pub unsafe extern "C" fn flow_scene_flat_in(scene: *mut FlowScene, look: *const FlowMaterial, points: *const f32, count: usize, normal: *const f32) -> c_int {
    guard(-1, || {
        let (Some(scene), Some(look), Some(normal)) = (scene.as_mut(), material(look), point(normal)) else { return -1 };
        let Some(points) = self::points(points, count).filter(|p| p.len() >= 3) else { return -1 };
        scene.0.flat(look, &points, normal);
        0
    })
}

#[no_mangle]
pub unsafe extern "C" fn flow_scene_triangles(scene: *const FlowScene) -> usize {
    guard(0, || scene.as_ref().map_or(0, |s| s.0.triangles()))
}

/// The scene as a GLB, its points stretched by (`squash_x`, `squash_y`)
/// into the terminal's picture, named as `generator` made it (NULL for
/// `flow-sdk`); its length in `len`.
#[no_mangle]
pub unsafe extern "C" fn flow_scene_glb(scene: *const FlowScene, squash_x: f32, squash_y: f32, generator: *const c_char, len: *mut usize) -> *mut u8 {
    scene_glb(scene, squash_x, squash_y, generator, len, false)
}

/// [`flow_scene_glb`] packed into whole numbers (KHR_mesh_quantization),
/// half the size, for Flow.
#[no_mangle]
pub unsafe extern "C" fn flow_scene_glb_packed(scene: *const FlowScene, squash_x: f32, squash_y: f32, generator: *const c_char, len: *mut usize) -> *mut u8 {
    scene_glb(scene, squash_x, squash_y, generator, len, true)
}

unsafe fn scene_glb(scene: *const FlowScene, squash_x: f32, squash_y: f32, generator: *const c_char, len: *mut usize, packed: bool) -> *mut u8 {
    guard(std::ptr::null_mut(), || {
        let (Some(scene), Some(len)) = (scene.as_ref(), len.as_mut()) else { return std::ptr::null_mut() };
        let squash = |k: f32| given(k).filter(|k| *k > 0.0);
        let (Some(sx), Some(sy)) = (squash(squash_x), squash(squash_y)) else { return std::ptr::null_mut() };
        let generator = if generator.is_null() { Some("flow-sdk") } else { text(generator) };
        let Some(generator) = generator else { return std::ptr::null_mut() };
        let glb = if packed { scene.0.glb_packed((sx, sy), generator) } else { scene.0.glb_by((sx, sy), generator) };
        let bytes = hand_over(&glb);
        *len = if bytes.is_null() { 0 } else { glb.len() };
        bytes
    })
}

/// A camera looking at `target` from `distance` away, turned `yaw` radians
/// to the right of looking from in front (+z) and `pitch` above it, seeing
/// `fov` radians up and down in a picture `height` pixels tall.
#[no_mangle]
pub unsafe extern "C" fn flow_camera_orbit(target: *const f32, yaw: f32, pitch: f32, distance: f32, fov: f32, height: f32) -> *mut FlowCamera {
    guard(std::ptr::null_mut(), || {
        let Some(target) = point(target) else { return std::ptr::null_mut() };
        if ![yaw, pitch, distance, fov, height].iter().all(|v| v.is_finite()) {
            return std::ptr::null_mut();
        }
        Box::into_raw(Box::new(FlowCamera(Camera::orbit(target, yaw, pitch, distance, fov, height))))
    })
}

#[no_mangle]
pub unsafe extern "C" fn flow_camera_free(camera: *mut FlowCamera) {
    if !camera.is_null() {
        guard((), || drop(Box::from_raw(camera)));
    }
}

/// Where point `p` of the world shows in the picture, into `out` (x, y and
/// the depth); -1 if it's behind the eye.
#[no_mangle]
pub unsafe extern "C" fn flow_camera_project(camera: *const FlowCamera, p: *const f32, out: *mut f32) -> c_int {
    guard(-1, || {
        let (Some(camera), Some(p)) = (camera.as_ref(), point(p)) else { return -1 };
        match camera.0.project(p) {
            Some(at) if put(out, at) => 0,
            _ => -1,
        }
    })
}

/// Direction `n` of the world as the picture has it, into `out`: x right,
/// y up, z toward the eye.
#[no_mangle]
pub unsafe extern "C" fn flow_camera_turn(camera: *const FlowCamera, n: *const f32, out: *mut f32) -> c_int {
    guard(-1, || {
        let (Some(camera), Some(n)) = (camera.as_ref(), point(n)) else { return -1 };
        if put(out, camera.0.turn(n)) {
            0
        } else {
            -1
        }
    })
}

/// A flat face of the world, `count` points turned `normal` way, drawn
/// into `scene` if it faces the eye and is in front of it (0 either way).
#[no_mangle]
pub unsafe extern "C" fn flow_camera_face(camera: *const FlowCamera, scene: *mut FlowScene, rgba: *const f32, points: *const f32, count: usize, normal: *const f32) -> c_int {
    guard(-1, || {
        let (Some(camera), Some(scene), Some(rgba), Some(normal)) = (camera.as_ref(), scene.as_mut(), color(rgba), point(normal)) else { return -1 };
        let Some(points) = self::points(points, count).filter(|p| p.len() >= 3) else { return -1 };
        camera.0.face(&mut scene.0, rgba, &points, normal);
        0
    })
}

/// [`flow_camera_face`] in a material.
#[no_mangle]
pub unsafe extern "C" fn flow_camera_face_in(camera: *const FlowCamera, scene: *mut FlowScene, look: *const FlowMaterial, points: *const f32, count: usize, normal: *const f32) -> c_int {
    guard(-1, || {
        let (Some(camera), Some(scene), Some(look), Some(normal)) = (camera.as_ref(), scene.as_mut(), material(look), point(normal)) else { return -1 };
        let Some(points) = self::points(points, count).filter(|p| p.len() >= 3) else { return -1 };
        camera.0.face(&mut scene.0, look, &points, normal);
        0
    })
}

/// The color to give a face turned `normal` way (x right, y up, z toward
/// the viewer) for it to show as `rgba` once Flow's light falls on it, into
/// `out`, which may be `rgba` itself.
#[no_mangle]
pub unsafe extern "C" fn flow_lit_as(rgba: *const f32, normal: *const f32, out: *mut f32) -> c_int {
    guard(-1, || {
        let (Some(c), Some(n)) = (color(rgba), point(normal)) else { return -1 };
        done(put(out, scene::lit_as(c, n)))
    })
}

// The vector maths a scene is worked out with, as `flow_sdk::scene` has it:
// points and directions of three floats, rotations of nine (by rows),
// colors of four. Each result goes where `out` points, which may be one of
// the arguments, since every argument is read before it's written.

/// Nine floats, a rotation by rows.
unsafe fn matrix(ptr: *const f32) -> Option<M3> {
    let m = slice(ptr, 9)?;
    Some([[m[0], m[1], m[2]], [m[3], m[4], m[5]], [m[6], m[7], m[8]]])
}

/// A rotation's nine floats, by rows.
fn flat(m: M3) -> [f32; 9] {
    [m[0][0], m[0][1], m[0][2], m[1][0], m[1][1], m[1][2], m[2][0], m[2][1], m[2][2]]
}

/// 0 for written, -1 for nowhere to write it.
fn done(written: bool) -> c_int {
    if written {
        0
    } else {
        -1
    }
}

/// Two points' sum, into `out`.
#[no_mangle]
pub unsafe extern "C" fn flow_v3_add(a: *const f32, b: *const f32, out: *mut f32) -> c_int {
    guard(-1, || match (point(a), point(b)) {
        (Some(a), Some(b)) => done(put(out, scene::add(a, b))),
        _ => -1,
    })
}

/// `a` less `b`, into `out`.
#[no_mangle]
pub unsafe extern "C" fn flow_v3_sub(a: *const f32, b: *const f32, out: *mut f32) -> c_int {
    guard(-1, || match (point(a), point(b)) {
        (Some(a), Some(b)) => done(put(out, scene::sub(a, b))),
        _ => -1,
    })
}

/// `a` times `k`, into `out`.
#[no_mangle]
pub unsafe extern "C" fn flow_v3_mul(a: *const f32, k: f32, out: *mut f32) -> c_int {
    guard(-1, || point(a).map_or(-1, |a| done(put(out, scene::mul(a, k)))))
}

/// The dot product of `a` and `b`; NAN if either is NULL.
#[no_mangle]
pub unsafe extern "C" fn flow_v3_dot(a: *const f32, b: *const f32) -> f32 {
    guard(f32::NAN, || match (point(a), point(b)) {
        (Some(a), Some(b)) => scene::dot(a, b),
        _ => f32::NAN,
    })
}

/// The cross product of `a` and `b`, the right-hand way, into `out`.
#[no_mangle]
pub unsafe extern "C" fn flow_v3_cross(a: *const f32, b: *const f32, out: *mut f32) -> c_int {
    guard(-1, || match (point(a), point(b)) {
        (Some(a), Some(b)) => done(put(out, scene::cross(a, b))),
        _ => -1,
    })
}

/// `a` made a length of 1, into `out`: (0, 0, 1) for one too short to
/// have a way.
#[no_mangle]
pub unsafe extern "C" fn flow_v3_normalize(a: *const f32, out: *mut f32) -> c_int {
    guard(-1, || point(a).map_or(-1, |a| done(put(out, scene::normalize(a)))))
}

/// The point `t` of the way from `a` to `b`, into `out`.
#[no_mangle]
pub unsafe extern "C" fn flow_v3_lerp(a: *const f32, b: *const f32, t: f32, out: *mut f32) -> c_int {
    guard(-1, || match (point(a), point(b)) {
        (Some(a), Some(b)) => done(put(out, scene::lerp(a, b, t))),
        _ => -1,
    })
}

/// Eases from 0 to 1 as `t` goes from `a` to `b`, still at both ends.
#[no_mangle]
pub extern "C" fn flow_smooth(a: f32, b: f32, t: f32) -> f32 {
    guard(f32::NAN, || scene::smooth(a, b, t))
}

/// Turning by `angle` radians about `axis`, the right-hand way, into `out`
/// (nine floats, by rows).
#[no_mangle]
pub unsafe extern "C" fn flow_m3_rotation(axis: *const f32, angle: f32, out: *mut f32) -> c_int {
    guard(-1, || point(axis).map_or(-1, |axis| done(put(out, flat(scene::rotation(axis, angle))))))
}

/// Rotation `m` turning `v`, into `out`.
#[no_mangle]
pub unsafe extern "C" fn flow_m3_apply(m: *const f32, v: *const f32, out: *mut f32) -> c_int {
    guard(-1, || match (matrix(m), point(v)) {
        (Some(m), Some(v)) => done(put(out, scene::apply(m, v))),
        _ => -1,
    })
}

/// Turning by `b` and then by `a`, as one rotation, into `out`.
#[no_mangle]
pub unsafe extern "C" fn flow_m3_compose(a: *const f32, b: *const f32, out: *mut f32) -> c_int {
    guard(-1, || match (matrix(a), matrix(b)) {
        (Some(a), Some(b)) => done(put(out, flat(scene::compose(a, b)))),
        _ => -1,
    })
}

/// 0xRRGGBB as red, green, blue and alpha (1), from 0 to 1, into `out`.
#[no_mangle]
pub unsafe extern "C" fn flow_rgb(hex: u32, out: *mut f32) -> c_int {
    guard(-1, || done(put(out, scene::rgb(hex))))
}

/// The color `t` of the way from `a` to `b`, alpha too, into `out`.
#[no_mangle]
pub unsafe extern "C" fn flow_mix(a: *const f32, b: *const f32, t: f32, out: *mut f32) -> c_int {
    guard(-1, || match (color(a), color(b)) {
        (Some(a), Some(b)) => done(put(out, scene::mix(a, b, t))),
        _ => -1,
    })
}

/// How much of Flow's light a face turned `normal` way (x right, y up, z
/// toward the viewer) gets: what [`flow_lit_as`] divides by. NAN for NULL.
#[no_mangle]
pub unsafe extern "C" fn flow_shade(normal: *const f32) -> f32 {
    guard(f32::NAN, || point(normal).map_or(f32::NAN, scene::shade))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flow_sdk::scene::rgb;

    fn glb_of(scene: *const FlowScene, squash: (f32, f32), generator: *const c_char) -> Vec<u8> {
        let mut len = 0usize;
        // SAFETY: handed out by this crate with its length, freed once.
        unsafe {
            let bytes = flow_scene_glb(scene, squash.0, squash.1, generator, &mut len);
            assert!(!bytes.is_null());
            let glb = std::slice::from_raw_parts(bytes, len).to_vec();
            crate::flow_free(bytes.cast());
            glb
        }
    }

    #[test]
    fn a_scene_built_from_c_is_the_scene_built_in_rust() {
        let red = rgb(0xff0000);
        let shadow = [0.0f32, 0.0, 0.0, 0.4];
        let mut rust = Scene::new(200.0, 100.0);
        rust.flat(red, &[[0.0, 0.0, 5.0], [10.0, 0.0, 5.0], [10.0, 10.0, 5.0]], [0.0, 0.0, 1.0]);
        let shade = [Corner { at: [0.0, 0.0, -5.0], normal: [0.0, 0.0, 1.0] }, Corner { at: [-10.0, 0.0, -5.0], normal: [0.0, 1.0, 0.0] }, Corner { at: [-150.0, -10.0, -5.0], normal: [0.0, 0.0, 1.0] }];
        rust.polygon(shadow, &shade);
        let scene = flow_scene_new(200.0, 100.0);
        let points = [0.0f32, 0.0, 5.0, 10.0, 0.0, 5.0, 10.0, 10.0, 5.0];
        let corners: Vec<f32> = shade.iter().flat_map(|c| c.at.into_iter().chain(c.normal)).collect();
        unsafe {
            assert_eq!(flow_scene_flat(scene, red.as_ptr(), points.as_ptr(), 3, [0.0f32, 0.0, 1.0].as_ptr()), 0);
            assert_eq!(flow_scene_polygon(scene, shadow.as_ptr(), corners.as_ptr(), 3), 0);
            assert_eq!(flow_scene_triangles(scene), rust.triangles());
        }
        assert_eq!(glb_of(scene, (1.0, 1.0), std::ptr::null()), rust.glb((1.0, 1.0)));
        assert_eq!(glb_of(scene, (0.5, 0.9), c"a C program".as_ptr()), rust.glb_by((0.5, 0.9), "a C program"));
        unsafe { flow_scene_free(scene) };
    }

    #[test]
    fn a_scene_in_materials_from_c_is_the_rust_ones_packed_or_not() {
        let hull = flow_sdk::scene::Material::plain(rgb(0x101014)).gloss(0.9).metal(0.2).coat(0.6, 0.8);
        let beam = flow_sdk::scene::Material::plain([1.0, 0.3, 0.2, 0.5]).unlit().glow([1.0, 0.3, 0.2], 2.0);
        let (c_hull, c_beam) = (FlowMaterial::from(hull), FlowMaterial::from(beam));
        let points = [0.0f32, 0.0, 5.0, 10.0, 0.0, 5.0, 10.0, 10.0, 5.0];
        let shade = [Corner { at: [0.0, 0.0, -5.0], normal: [0.0, 0.0, 1.0] }, Corner { at: [-10.0, 0.0, -5.0], normal: [0.0, 1.0, 0.0] }, Corner { at: [-50.0, -10.0, -5.0], normal: [0.0, 0.0, 1.0] }];
        let corners: Vec<f32> = shade.iter().flat_map(|c| c.at.into_iter().chain(c.normal)).collect();
        let cam = Camera::orbit([0.0; 3], 0.0, 0.0, 6.0, 0.7, 100.0);
        let square = [[-1.0f32, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]];
        let flat: Vec<f32> = square.iter().flatten().copied().collect();
        let mut rust = Scene::new(200.0, 100.0);
        rust.flat(hull, &[[0.0, 0.0, 5.0], [10.0, 0.0, 5.0], [10.0, 10.0, 5.0]], [0.0, 0.0, 1.0]);
        rust.polygon(beam, &shade);
        cam.face(&mut rust, hull, &square, [0.0, 0.0, 1.0]);
        let scene = flow_scene_new(200.0, 100.0);
        let camera = unsafe { flow_camera_orbit([0.0f32; 3].as_ptr(), 0.0, 0.0, 6.0, 0.7, 100.0) };
        let mut len = 0usize;
        unsafe {
            assert_eq!(flow_scene_flat_in(scene, &c_hull, points.as_ptr(), 3, [0.0f32, 0.0, 1.0].as_ptr()), 0);
            assert_eq!(flow_scene_polygon_in(scene, &c_beam, corners.as_ptr(), 3), 0);
            assert_eq!(flow_camera_face_in(camera, scene, &c_hull, flat.as_ptr(), 4, [0.0f32, 0.0, 1.0].as_ptr()), 0);
            assert_eq!(glb_of(scene, (1.0, 1.0), std::ptr::null()), rust.glb((1.0, 1.0)));
            let packed = flow_scene_glb_packed(scene, 0.5, 0.9, c"a C program".as_ptr(), &mut len);
            assert_eq!(std::slice::from_raw_parts(packed, len), &rust.glb_packed((0.5, 0.9), "a C program")[..]);
            crate::flow_free(packed.cast());
            assert_eq!(flow_scene_flat_in(scene, std::ptr::null(), points.as_ptr(), 3, [0.0f32, 0.0, 1.0].as_ptr()), -1);
            assert_eq!(flow_scene_polygon_in(scene, &FlowMaterial { metal: f32::INFINITY, ..c_hull }, corners.as_ptr(), 3), -1);
            assert_eq!(flow_camera_face_in(std::ptr::null(), scene, &c_hull, flat.as_ptr(), 4, [0.0f32, 0.0, 1.0].as_ptr()), -1);
            assert!(flow_scene_glb_packed(scene, 0.0, 1.0, std::ptr::null(), &mut len).is_null(), "a squash above 0");
            flow_camera_free(camera);
            flow_scene_free(scene);
        }
    }

    #[test]
    fn the_camera_from_c_sees_what_the_camera_in_rust_does() {
        let rust = Camera::orbit([0.0, 0.5, 0.0], 0.4, 0.3, 6.0, 0.7, 400.0);
        let camera = unsafe { flow_camera_orbit([0.0f32, 0.5, 0.0].as_ptr(), 0.4, 0.3, 6.0, 0.7, 400.0) };
        let mut out = [0.0f32; 3];
        unsafe {
            assert_eq!(flow_camera_project(camera, [1.0f32, 1.0, 1.0].as_ptr(), out.as_mut_ptr()), 0);
            assert_eq!(Some(out), rust.project([1.0, 1.0, 1.0]));
            assert_eq!(flow_camera_project(camera, [10.0f32, 10.0, 10.0].as_ptr(), out.as_mut_ptr()), -1, "behind the eye");
            assert_eq!(flow_camera_turn(camera, [0.0f32, 0.0, 1.0].as_ptr(), out.as_mut_ptr()), 0);
            assert_eq!(out, rust.turn([0.0, 0.0, 1.0]));
        }
        let square = [[-1.0f32, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]];
        let flat: Vec<f32> = square.iter().flatten().copied().collect();
        let mut theirs = Scene::new(800.0, 600.0);
        for normal in [[0.0, 0.0, 1.0], [0.0, 0.0, -1.0]] {
            rust.face(&mut theirs, rgb(0x7ce7ff), &square, normal);
        }
        let scene = flow_scene_new(800.0, 600.0);
        unsafe {
            for normal in [[0.0f32, 0.0, 1.0], [0.0, 0.0, -1.0]] {
                assert_eq!(flow_camera_face(camera, scene, rgb(0x7ce7ff).as_ptr(), flat.as_ptr(), 4, normal.as_ptr()), 0);
            }
            assert_eq!(flow_scene_triangles(scene), 2, "the face turned away isn't drawn");
        }
        assert_eq!(glb_of(scene, (1.0, 1.0), std::ptr::null()), theirs.glb((1.0, 1.0)));
        unsafe {
            flow_scene_free(scene);
            flow_camera_free(camera);
        }
    }

    #[test]
    fn a_face_lit_from_c_takes_the_color_rust_gives_it_in_place() {
        let mut c = rgb(0x8040c0);
        let n = [0.3f32, 0.5, 0.8];
        unsafe { assert_eq!(flow_lit_as(c.as_ptr(), n.as_ptr(), c.as_mut_ptr()), 0) };
        assert_eq!(c, scene::lit_as(rgb(0x8040c0), n));
    }

    #[test]
    fn the_vector_maths_from_c_is_the_sdks() {
        let (a, b) = ([1.0f32, 2.0, 3.0], [-4.0f32, 0.5, 2.0]);
        let mut out = [0.0f32; 3];
        unsafe {
            assert_eq!(flow_v3_add(a.as_ptr(), b.as_ptr(), out.as_mut_ptr()), 0);
            assert_eq!(out, scene::add(a, b));
            flow_v3_sub(a.as_ptr(), b.as_ptr(), out.as_mut_ptr());
            assert_eq!(out, scene::sub(a, b));
            flow_v3_mul(a.as_ptr(), 2.5, out.as_mut_ptr());
            assert_eq!(out, scene::mul(a, 2.5));
            assert_eq!(flow_v3_dot(a.as_ptr(), b.as_ptr()), scene::dot(a, b));
            flow_v3_cross(a.as_ptr(), b.as_ptr(), out.as_mut_ptr());
            assert_eq!(out, scene::cross(a, b));
            flow_v3_lerp(a.as_ptr(), b.as_ptr(), 0.25, out.as_mut_ptr());
            assert_eq!(out, scene::lerp(a, b, 0.25));
            // In place: the result written over its own argument.
            out = a;
            flow_v3_normalize(out.as_ptr(), out.as_mut_ptr());
            assert_eq!(out, scene::normalize(a));
            flow_v3_normalize([0.0f32; 3].as_ptr(), out.as_mut_ptr());
            assert_eq!(out, [0.0, 0.0, 1.0], "too short to have a way");
        }
        assert_eq!(flow_smooth(0.0, 2.0, 0.5), scene::smooth(0.0, 2.0, 0.5));
        let (spin, tilt) = (scene::rotation([0.0, 1.0, 0.0], 0.7), scene::rotation([1.0, 0.2, 0.0], -0.4));
        let (mut m, mut n) = ([0.0f32; 9], [0.0f32; 9]);
        unsafe {
            assert_eq!(flow_m3_rotation([0.0f32, 1.0, 0.0].as_ptr(), 0.7, m.as_mut_ptr()), 0);
            assert_eq!(m, flat(spin));
            flow_m3_rotation([1.0f32, 0.2, 0.0].as_ptr(), -0.4, n.as_mut_ptr());
            assert_eq!(n, flat(tilt));
            flow_m3_apply(m.as_ptr(), a.as_ptr(), out.as_mut_ptr());
            assert_eq!(out, scene::apply(spin, a));
            flow_m3_compose(m.as_ptr(), n.as_ptr(), m.as_mut_ptr());
            assert_eq!(m, flat(scene::compose(spin, tilt)), "in place, as one turn after the other");
        }
        let mut c = [0.0f32; 4];
        unsafe {
            assert_eq!(flow_rgb(0x8040c0, c.as_mut_ptr()), 0);
            assert_eq!(c, scene::rgb(0x8040c0));
            flow_mix(c.as_ptr(), [1.0f32, 1.0, 1.0, 0.0].as_ptr(), 0.5, c.as_mut_ptr());
            assert_eq!(c, scene::mix(scene::rgb(0x8040c0), [1.0, 1.0, 1.0, 0.0], 0.5));
            assert_eq!(flow_shade([0.3f32, 0.5, 0.8].as_ptr()), scene::shade([0.3, 0.5, 0.8]));
        }
    }

    #[test]
    fn the_vector_maths_refuses_null_pointers() {
        let p = [1.0f32; 9];
        let mut out = [0.0f32; 9];
        let null = std::ptr::null::<f32>();
        unsafe {
            assert_eq!(flow_v3_add(null, p.as_ptr(), out.as_mut_ptr()), -1);
            assert_eq!(flow_v3_sub(p.as_ptr(), p.as_ptr(), std::ptr::null_mut()), -1);
            assert_eq!(flow_v3_mul(null, 2.0, out.as_mut_ptr()), -1);
            assert!(flow_v3_dot(p.as_ptr(), null).is_nan());
            assert_eq!(flow_v3_cross(p.as_ptr(), null, out.as_mut_ptr()), -1);
            assert_eq!(flow_v3_normalize(null, out.as_mut_ptr()), -1);
            assert_eq!(flow_v3_lerp(null, p.as_ptr(), 0.5, out.as_mut_ptr()), -1);
            assert_eq!(flow_m3_rotation(null, 1.0, out.as_mut_ptr()), -1);
            assert_eq!(flow_m3_apply(null, p.as_ptr(), out.as_mut_ptr()), -1);
            assert_eq!(flow_m3_compose(p.as_ptr(), null, out.as_mut_ptr()), -1);
            assert_eq!(flow_rgb(0xffffff, std::ptr::null_mut()), -1);
            assert_eq!(flow_mix(p.as_ptr(), null, 0.5, out.as_mut_ptr()), -1);
            assert!(flow_shade(null).is_nan());
        }
        assert_eq!(out, [0.0; 9], "nothing written");
    }

    #[test]
    fn what_a_scene_cant_take_is_refused() {
        assert!(flow_scene_new(0.0, 100.0).is_null());
        assert!(flow_scene_new(f32::NAN, 100.0).is_null());
        let scene = flow_scene_new(100.0, 100.0);
        let white = [1.0f32; 4];
        let two = [0.0f32; 12];
        let mut len = 7usize;
        unsafe {
            assert_eq!(flow_scene_polygon(scene, white.as_ptr(), two.as_ptr(), 2), -1, "two corners");
            assert_eq!(flow_scene_flat(scene, white.as_ptr(), two.as_ptr(), 2, [0.0f32, 0.0, 1.0].as_ptr()), -1);
            assert_eq!(flow_scene_flat(scene, white.as_ptr(), std::ptr::null(), 3, [0.0f32, 0.0, 1.0].as_ptr()), -1);
            assert_eq!(flow_scene_flat(std::ptr::null_mut(), white.as_ptr(), two.as_ptr(), 4, [0.0f32, 0.0, 1.0].as_ptr()), -1);
            assert_eq!(flow_scene_triangles(std::ptr::null()), 0);
            assert!(flow_scene_glb(scene, 0.0, 1.0, std::ptr::null(), &mut len).is_null(), "a squash of 0");
            assert!(flow_scene_glb(scene, 1.0, 1.0, c"\xff".as_ptr(), &mut len).is_null(), "a name that isn't UTF-8");
            assert!(flow_scene_glb(scene, 1.0, 1.0, std::ptr::null(), std::ptr::null_mut()).is_null());
            assert!(flow_camera_orbit(std::ptr::null(), 0.0, 0.0, 6.0, 0.7, 400.0).is_null());
            assert!(flow_camera_orbit([0.0f32; 3].as_ptr(), 0.0, f32::INFINITY, 6.0, 0.7, 400.0).is_null());
            assert_eq!(flow_camera_turn(std::ptr::null(), [0.0f32; 3].as_ptr(), [0.0f32; 3].as_mut_ptr()), -1);
            assert_eq!(flow_lit_as(white.as_ptr(), [0.0f32, 0.0, 1.0].as_ptr(), std::ptr::null_mut()), -1);
            flow_scene_free(scene);
            flow_scene_free(std::ptr::null_mut());
            flow_camera_free(std::ptr::null_mut());
        }
    }
}
