//! An object as triangles in its own space (y up), for the Ratty graphics
//! protocol ([`crate::model`]): a thing the terminal turns, lights and
//! animates itself, as a cut gem or a crown is, rather than a
//! [`crate::scene::Scene`], a picture the program works out afresh each
//! frame. The terminal centers it in a box whose longest side is 1, so its
//! size here is only its shape.
//!
//! Each face is flat and convex, given with a point it faces away from (the
//! middle of the object, for one that's solid), and wound as glTF wants:
//! counter-clockwise seen from outside, since Flow lights a triangle wound
//! the other way as its back.
//!
//! A face takes a color or a [`Material`]: polished, metal, glowing,
//! clear-coated or unlit, as a scene's polygons do.

use crate::scene::{cross, dot, normalize, part_in, sub, write_glb, Material, Part, V3};

/// An object being built, face by face, to send as a GLB.
#[derive(Debug, Default)]
pub struct Mesh {
    parts: Vec<Part>,
}

impl Mesh {
    /// An object with no faces yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// A flat convex face of `points` in a color or a [`Material`],
    /// facing away from `inside`.
    pub fn face(&mut self, material: impl Into<Material>, points: &[V3], inside: V3) {
        let material = material.into();
        if points.len() < 3 || material.color[3] <= 0.0 {
            return;
        }
        // Its normal by Newell's sum, which a slightly bent face can't fool.
        let mut n = [0.0f32; 3];
        for (i, a) in points.iter().enumerate() {
            let b = points[(i + 1) % points.len()];
            n[0] += (a[1] - b[1]) * (a[2] + b[2]);
            n[1] += (a[2] - b[2]) * (a[0] + b[0]);
            n[2] += (a[0] - b[0]) * (a[1] + b[1]);
        }
        let mut n = normalize(n);
        let middle = points.iter().fold([0.0; 3], |m, p| [m[0] + p[0], m[1] + p[1], m[2] + p[2]]);
        let middle = [middle[0] / points.len() as f32, middle[1] / points.len() as f32, middle[2] / points.len() as f32];
        let mut points = points.to_vec();
        if dot(n, sub(middle, inside)) < 0.0 {
            points.reverse();
            n = [-n[0], -n[1], -n[2]];
        }
        let part = part_in(&mut self.parts, material);
        let base = part.pos.len() as u32;
        for p in &points {
            part.pos.push(*p);
            part.normal.push(n);
        }
        for i in 1..points.len() as u32 - 1 {
            part.index.extend([base, base + i, base + i + 1]);
        }
    }

    /// A triangle facing away from `inside`.
    pub fn triangle(&mut self, material: impl Into<Material>, a: V3, b: V3, c: V3, inside: V3) {
        self.face(material, &[a, b, c], inside);
    }

    /// How many triangles its faces make.
    pub fn triangles(&self) -> usize {
        self.parts.iter().map(|p| p.index.len() / 3).sum()
    }

    /// The object as a GLB, a primitive for each material.
    pub fn glb(&self) -> Vec<u8> {
        write_glb(&self.parts, "flow-sdk", false)
    }

    /// [`Mesh::glb`] with its points and normals packed into whole numbers
    /// (glTF's KHR_mesh_quantization, which the file requires): half the
    /// size, for Flow and terminals that read it.
    pub fn glb_packed(&self) -> Vec<u8> {
        write_glb(&self.parts, "flow-sdk", true)
    }

    /// Whether each triangle winds counter-clockwise about its normal, as
    /// seen from outside: for tests of what's built.
    pub fn wound_outward(&self) -> bool {
        self.parts.iter().all(|p| {
            p.index.chunks(3).all(|t| {
                let (a, b, c) = (p.pos[t[0] as usize], p.pos[t[1] as usize], p.pos[t[2] as usize]);
                dot(cross(sub(b, a), sub(c, a)), p.normal[t[0] as usize]) > 0.0
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_face_turns_to_wind_outward_whichever_way_it_was_given() {
        let mut m = Mesh::new();
        let gold = [0.9, 0.7, 0.2, 1.0];
        // The same square, given either way round, faces away from the middle.
        m.face(gold, &[[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0]], [0.5, 0.5, 0.0]);
        m.face(gold, &[[0.0, 1.0, -1.0], [1.0, 1.0, -1.0], [1.0, 0.0, -1.0], [0.0, 0.0, -1.0]], [0.5, 0.5, 0.0]);
        assert_eq!(m.triangles(), 4);
        assert!(m.wound_outward());
        let glb = m.glb();
        assert_eq!(&glb[..4], b"glTF");
        assert_eq!(u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize, glb.len());
    }
}
