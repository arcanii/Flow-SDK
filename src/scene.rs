//! A picture as triangles, sent to the terminal as one binary glTF (GLB)
//! with the Ratty graphics protocol ([`crate::model`]). The program works
//! out where everything is itself, in the picture's pixels (from its
//! middle, y up), and the terminal draws and lights it (in Flow, its
//! renderer for the Ratty graphics protocol). Placements that overlap can't
//! share cells, so a scene of several things moving apart is one object,
//! sent anew each frame ([`crate::stage`]).
//!
//! Polygons are convex, cut to the picture, and wound counter-clockwise as
//! the screen shows them, since Flow lights a triangle wound the other way
//! as its back. The terminal centers a model in a box whose longest side is
//! 1 and fits that box to the placement, so each GLB also carries four
//! invisible corners (their alpha 0, which Flow's shader discards) at the
//! picture's corners: the box is then the picture, and every point lands
//! where the scene put it. A [`Camera`] turns a world in 3D into the
//! picture's pixels, in perspective.
//!
//! A polygon takes a color, or a [`Material`]: the color with a polish, a
//! metal, a glow of its own, a clear coat over it, or none of the light at
//! all, which Flow draws as glTF's materials say. A plain color is matte,
//! as every part was before materials, and is written as it always was.
//! [`Scene::glb_packed`] packs the points and normals into whole numbers
//! (glTF's KHR_mesh_quantization), half the size, for a terminal that
//! reads them as Flow does.

use std::fmt::Write as _;

/// A point or a direction: x, y and z.
pub type V3 = [f32; 3];

/// `a` + `b`.
pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// `a` less `b`.
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `a` times `k`.
pub fn mul(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// The dot product of `a` and `b`.
pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The cross product of `a` and `b`, the right-hand way.
pub fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// `a` made a length of 1; (0, 0, 1) for one too short to have a way.
pub fn normalize(a: V3) -> V3 {
    let len = dot(a, a).sqrt();
    if len < 1e-9 {
        [0.0, 0.0, 1.0]
    } else {
        mul(a, 1.0 / len)
    }
}

/// The point `t` of the way from `a` to `b`.
pub fn lerp(a: V3, b: V3, t: f32) -> V3 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Eases from 0 to 1 as `t` goes from `a` to `b`, still at both ends.
pub fn smooth(a: f32, b: f32, t: f32) -> f32 {
    let x = ((t - a) / (b - a)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// A rotation, by rows.
pub type M3 = [[f32; 3]; 3];

/// No turn at all.
pub const IDENTITY: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Turning by `angle` radians about `axis`, the right-hand way.
pub fn rotation(axis: V3, angle: f32) -> M3 {
    let [x, y, z] = normalize(axis);
    let (s, c) = angle.sin_cos();
    let k = 1.0 - c;
    [
        [c + x * x * k, x * y * k - z * s, x * z * k + y * s],
        [y * x * k + z * s, c + y * y * k, y * z * k - x * s],
        [z * x * k - y * s, z * y * k + x * s, c + z * z * k],
    ]
}

/// Rotation `m` turning `v`.
pub fn apply(m: M3, v: V3) -> V3 {
    [dot(m[0], v), dot(m[1], v), dot(m[2], v)]
}

/// `a` after `b`: turning by `b` and then by `a`, as one rotation.
pub fn compose(a: M3, b: M3) -> M3 {
    let col = |j: usize| [b[0][j], b[1][j], b[2][j]];
    let row = |i: usize| [dot(a[i], col(0)), dot(a[i], col(1)), dot(a[i], col(2))];
    [row(0), row(1), row(2)]
}

/// A color as shown: red, green, blue and alpha, from 0 to 1.
pub type Color = [f32; 4];

/// 0xRRGGBB as a color, opaque.
pub fn rgb(hex: u32) -> Color {
    let c = |shift: u32| ((hex >> shift) & 0xff) as f32 / 255.0;
    [c(16), c(8), c(0), 1.0]
}

/// The color `t` of the way from `a` to `b`, alpha too.
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    [0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * t)
}

/// Where Flow's shader has the light come from (its `render/models.rs`):
/// above, left and in front. Other terminals may light models otherwise.
const LIGHT: V3 = [-0.45, 0.65, 0.61];

/// How much light a face turned `n` way (x right, y up, z toward the
/// viewer) gets in Flow: some all round, the light's, and a rim at the edges.
pub fn shade(n: V3) -> f32 {
    let n = normalize(n);
    let diffuse = dot(n, normalize(LIGHT)).max(0.0);
    0.35 + 0.7 * diffuse + (1.0 - n[2].abs()).powi(3) * 0.25
}

/// The color to give a face turned `n` way for it to show as `c` once lit.
pub fn lit_as(c: Color, n: V3) -> Color {
    let k = shade(n);
    [(c[0] / k).min(1.0), (c[1] / k).min(1.0), (c[2] / k).min(1.0), c[3]]
}

/// How a surface looks: its color, and how the light falls on it. A plain
/// color ([`Material::plain`], or a [`Color`] given where a material goes)
/// is matte and lit, as Flow lights a part with no more said; the rest is
/// glTF's metallic-roughness material and the extensions Flow reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    /// Its color, red, green, blue and alpha from 0 to 1 as shown; alpha
    /// under 1 is see-through, drawn after what's solid.
    pub color: Color,
    /// How polished it is, 0 (matte, glTF's own default) to 1 (a mirror's
    /// shine): one less glTF's roughness. A polished surface takes Flow's
    /// highlights and its room's reflections.
    pub gloss: f32,
    /// How much a metal it is, 0 to 1: a metal's reflections take its
    /// color, and a rough one hardly shows it.
    pub metal: f32,
    /// The light it gives off, red, green and blue from 0 to 1 as shown,
    /// black for none, whatever falls on it.
    pub glow: [f32; 3],
    /// How strong its glow is, 1 as given (glTF's emissive strength,
    /// which may go past 1).
    pub glow_strength: f32,
    /// A clear coat over it, 0 (none) to 1: a glaze of its own reflections
    /// over the color and the metal under it.
    pub coat: f32,
    /// How polished the clear coat is, 0 to 1.
    pub coat_gloss: f32,
    /// Drawn in its color alone, no light falling on it (glTF's
    /// KHR_materials_unlit): for what shines, as a beam does.
    pub unlit: bool,
}

impl Material {
    /// `color`, matte and lit: what a color alone is.
    pub fn plain(color: Color) -> Self {
        Self { color, gloss: 0.0, metal: 0.0, glow: [0.0; 3], glow_strength: 1.0, coat: 0.0, coat_gloss: 0.0, unlit: false }
    }

    /// Polished to `gloss`, 0 to 1.
    pub fn gloss(self, gloss: f32) -> Self {
        Self { gloss, ..self }
    }

    /// A metal as far as `metal`, 0 to 1.
    pub fn metal(self, metal: f32) -> Self {
        Self { metal, ..self }
    }

    /// Giving off light `glow` (red, green and blue, 0 to 1 as shown) as
    /// strong as `strength`, 1 as given.
    pub fn glow(self, glow: [f32; 3], strength: f32) -> Self {
        Self { glow, glow_strength: strength, ..self }
    }

    /// A clear coat over it as strong as `coat` and polished to `gloss`,
    /// each 0 to 1.
    pub fn coat(self, coat: f32, gloss: f32) -> Self {
        Self { coat, coat_gloss: gloss, ..self }
    }

    /// Drawn in its color alone, no light falling on it.
    pub fn unlit(self) -> Self {
        Self { unlit: true, ..self }
    }

    /// Whether it's only a color, matte and lit, written as a color alone
    /// always was.
    pub fn is_plain(&self) -> bool {
        Self { color: self.color, ..Self::plain(self.color) } == *self
    }
}

impl From<Color> for Material {
    fn from(color: Color) -> Self {
        Self::plain(color)
    }
}

/// A polygon's corner: where it is in the picture (pixels from the center,
/// y up; z only orders what's in front, nearer being more) and which way
/// its surface faces there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Corner {
    /// Where it is.
    pub at: V3,
    /// Which way the surface faces there.
    pub normal: V3,
}

/// Triangles of one material.
#[derive(Debug)]
pub(crate) struct Part {
    pub(crate) material: Material,
    pub(crate) pos: Vec<V3>,
    pub(crate) normal: Vec<V3>,
    pub(crate) index: Vec<u32>,
}

impl Part {
    pub(crate) fn new(material: Material) -> Self {
        Self { material, pos: Vec::new(), normal: Vec::new(), index: Vec::new() }
    }
}

/// The part of `parts` in `material`, made if there's none yet.
pub(crate) fn part_in(parts: &mut Vec<Part>, material: Material) -> &mut Part {
    match parts.iter().position(|p| p.material == material) {
        Some(i) => &mut parts[i],
        None => {
            parts.push(Part::new(material));
            parts.last_mut().expect("just pushed")
        }
    }
}

/// A frame's picture, `width` × `height` pixels as the screen shows them.
pub struct Scene {
    /// Its width in the screen's pixels.
    pub width: f32,
    /// Its height in the screen's pixels.
    pub height: f32,
    parts: Vec<Part>,
}

impl Scene {
    /// An empty picture `width` × `height` pixels: a stage's
    /// ([`crate::stage::Stage::width`], [`crate::stage::Stage::height`]).
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height, parts: Vec::new() }
    }

    /// A flat convex polygon in a color or a [`Material`], cut to the
    /// picture: what lies outside it would make the box bigger, and the
    /// whole picture smaller.
    pub fn polygon(&mut self, material: impl Into<Material>, corners: &[Corner]) {
        let material = material.into();
        let mut corners = clip(corners.to_vec(), self.width * 0.5, self.height * 0.5);
        if corners.len() < 3 || material.color[3] <= 0.0 {
            return;
        }
        // Flow lights a triangle that winds clockwise on the screen as its
        // back, so every polygon goes the other way round.
        let n = corners.len();
        let twice_area: f32 = (0..n).map(|i| (corners[i].at, corners[(i + 1) % n].at)).map(|(a, b)| a[0] * b[1] - b[0] * a[1]).sum();
        if twice_area < 0.0 {
            corners.reverse();
        }
        let part = part_in(&mut self.parts, material);
        let base = part.pos.len() as u32;
        for c in &corners {
            part.pos.push(c.at);
            part.normal.push(c.normal);
        }
        for i in 1..corners.len() as u32 - 1 {
            part.index.extend([base, base + i, base + i + 1]);
        }
    }

    /// A polygon facing one way all over.
    pub fn flat(&mut self, material: impl Into<Material>, points: &[V3], normal: V3) {
        let corners: Vec<Corner> = points.iter().map(|&at| Corner { at, normal }).collect();
        self.polygon(material, &corners);
    }

    /// How many triangles its polygons make.
    pub fn triangles(&self) -> usize {
        self.parts.iter().map(|p| p.index.len() / 3).sum()
    }

    /// The scene as a GLB, its points stretched by `squash` into the
    /// terminal's picture: that's a cell's whole pixels per cell, and the
    /// screen shows it over a cell's real width and height.
    pub fn glb(&self, squash: (f32, f32)) -> Vec<u8> {
        self.glb_by(squash, "flow-sdk")
    }

    /// [`Scene::glb`], naming the program that made it in glTF's
    /// `asset.generator`.
    pub fn glb_by(&self, squash: (f32, f32), generator: &str) -> Vec<u8> {
        write_glb(&self.squashed(squash), generator, false)
    }

    /// [`Scene::glb_by`] with its points and normals packed into whole
    /// numbers (glTF's KHR_mesh_quantization, which the file requires):
    /// half the size, for Flow and terminals that read it. A point lands
    /// within a sixty-thousandth of the picture's width of where it was.
    pub fn glb_packed(&self, squash: (f32, f32), generator: &str) -> Vec<u8> {
        write_glb(&self.squashed(squash), generator, true)
    }

    /// Its parts squashed into the terminal's picture, with the picture's
    /// invisible corners.
    fn squashed(&self, (sx, sy): (f32, f32)) -> Vec<Part> {
        let squashed = self.parts.iter().map(|p| Part {
            material: p.material,
            pos: p.pos.iter().map(|v| [v[0] * sx, v[1] * sy, v[2]]).collect(),
            normal: p.normal.iter().map(|n| normalize([n[0] / sx, n[1] / sy, n[2]])).collect(),
            index: p.index.clone(),
        });
        let (w, h) = (self.width * sx * 0.5, self.height * sy * 0.5);
        let corners = Part { material: Material::plain([0.0; 4]), pos: vec![[-w, -h, 0.0], [w, -h, 0.0], [w, h, 0.0], [-w, h, 0.0]], normal: vec![[0.0, 0.0, 1.0]; 4], index: vec![0, 1, 2, 0, 2, 3] };
        squashed.chain([corners]).collect()
    }
}

/// A number as JSON has it: 0 for one that isn't finite.
fn num(f: f32) -> String {
    if f.is_finite() {
        format!("{f}")
    } else {
        "0".into()
    }
}

/// A color as shown, 0 to 1, as glTF's linear light.
fn linear(c: f32) -> String {
    num(c.clamp(0.0, 1.0).powf(2.2))
}

/// The extensions a material uses, as glTF names them.
fn extensions_of(m: &Material) -> Vec<&'static str> {
    let mut used = Vec::new();
    if m.glow.iter().any(|&c| c > 0.0) && m.glow_strength != 1.0 {
        used.push("KHR_materials_emissive_strength");
    }
    if m.unlit {
        used.push("KHR_materials_unlit");
    }
    if m.coat > 0.0 {
        used.push("KHR_materials_clearcoat");
    }
    used
}

/// A material as glTF's JSON: a plain color as it always was (matte, as
/// glTF's roughness of 1 by default is), anything more with the factors
/// and extensions that say it.
fn material_json(m: &Material) -> String {
    // glTF's factors are linear light; Flow shows them as display values.
    let [r, g, b, alpha] = m.color;
    let mode = if alpha < 1.0 { "BLEND" } else { "OPAQUE" };
    let base = format!("\"baseColorFactor\":[{},{},{},{}]", linear(r), linear(g), linear(b), num(alpha.clamp(0.0, 1.0)));
    if m.is_plain() {
        return format!("{{\"pbrMetallicRoughness\":{{{base},\"metallicFactor\":0}},\"alphaMode\":\"{mode}\",\"doubleSided\":true}}");
    }
    let mut json = format!("{{\"pbrMetallicRoughness\":{{{base},\"metallicFactor\":{}", num(m.metal.clamp(0.0, 1.0)));
    if m.gloss > 0.0 {
        let _ = write!(json, ",\"roughnessFactor\":{}", num(1.0 - m.gloss.clamp(0.0, 1.0)));
    }
    json.push('}');
    let glows = m.glow.iter().any(|&c| c > 0.0);
    if glows {
        let _ = write!(json, ",\"emissiveFactor\":[{},{},{}]", linear(m.glow[0]), linear(m.glow[1]), linear(m.glow[2]));
    }
    let used = extensions_of(m);
    if !used.is_empty() {
        let each: Vec<String> = used
            .iter()
            .map(|&name| match name {
                "KHR_materials_emissive_strength" => format!("\"{name}\":{{\"emissiveStrength\":{}}}", num(m.glow_strength.max(0.0))),
                "KHR_materials_clearcoat" => format!("\"{name}\":{{\"clearcoatFactor\":{},\"clearcoatRoughnessFactor\":{}}}", num(m.coat.clamp(0.0, 1.0)), num(1.0 - m.coat_gloss.clamp(0.0, 1.0))),
                _ => format!("\"{name}\":{{}}"),
            })
            .collect();
        let _ = write!(json, ",\"extensions\":{{{}}}", each.join(","));
    }
    let _ = write!(json, ",\"alphaMode\":\"{mode}\",\"doubleSided\":true}}");
    json
}

/// Where a packed mesh's numbers are measured from, and how far a whole
/// number's range reaches either way: one scale for every axis, so the
/// normals, which the node's scale turns as its inverse, keep their way.
fn packing(parts: &[Part]) -> (V3, f32) {
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for v in parts.iter().flat_map(|p| &p.pos) {
        for k in 0..3 {
            lo[k] = lo[k].min(v[k]);
            hi[k] = hi[k].max(v[k]);
        }
    }
    if lo[0] > hi[0] {
        return ([0.0; 3], 1.0);
    }
    let middle = [0, 1, 2].map(|k| (lo[k] + hi[k]) * 0.5);
    let reach = (0..3).map(|k| (hi[k] - lo[k]) * 0.5).fold(0.0f32, f32::max);
    (middle, if reach > 1e-12 && reach.is_finite() { reach } else { 1.0 })
}

/// `parts` as a GLB: a mesh of one primitive a part, each in its part's
/// material, two-sided, named as `generator` made it. Indices are 16-bit
/// where a part's vertices allow, and with `packed` its points are 16-bit
/// and its normals 8-bit whole numbers (KHR_mesh_quantization), which its
/// node scales back.
pub(crate) fn write_glb(parts: &[Part], generator: &str, packed: bool) -> Vec<u8> {
    let mut bin: Vec<u8> = Vec::new();
    let (mut views, mut accessors, mut materials, mut primitives) = (String::new(), String::new(), String::new(), String::new());
    let (middle, reach) = packing(parts);
    let mut used: Vec<&str> = if packed { vec!["KHR_mesh_quantization"] } else { Vec::new() };
    for (i, p) in parts.iter().enumerate() {
        let sep = if i == 0 { "" } else { "," };
        // Each view starts on four bytes, as glTF wants of its attributes.
        let mut view = |bytes: Vec<u8>, stride: Option<usize>| {
            let stride = stride.map_or(String::new(), |s| format!(",\"byteStride\":{s}"));
            let _ = write!(views, "{}{{\"buffer\":0,\"byteOffset\":{},\"byteLength\":{}{stride}}}", if bin.is_empty() { "" } else { "," }, bin.len(), bytes.len());
            bin.extend(bytes);
            bin.resize(bin.len().div_ceil(4) * 4, 0);
        };
        let (n, a) = (p.pos.len(), 3 * i);
        let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        if packed {
            // A point as shorts from the middle, padded to eight bytes; a
            // normal as bytes, padded to four.
            let short = |v: f32| (v.clamp(-1.0, 1.0) * 32767.0).round() as i16;
            let points: Vec<[i16; 3]> = p.pos.iter().map(|v| [0, 1, 2].map(|k| short((v[k] - middle[k]) / reach))).collect();
            for q in &points {
                for k in 0..3 {
                    lo[k] = lo[k].min(f32::from(q[k]));
                    hi[k] = hi[k].max(f32::from(q[k]));
                }
            }
            view(points.iter().flat_map(|q| [q[0], q[1], q[2], 0]).flat_map(i16::to_le_bytes).collect(), Some(8));
            let byte = |v: f32| (v.clamp(-1.0, 1.0) * 127.0).round() as i8 as u8;
            view(p.normal.iter().flat_map(|n| [byte(n[0]), byte(n[1]), byte(n[2]), 0]).collect(), Some(4));
        } else {
            for v in &p.pos {
                for k in 0..3 {
                    lo[k] = lo[k].min(v[k]);
                    hi[k] = hi[k].max(v[k]);
                }
            }
            view(p.pos.iter().flatten().flat_map(|f| f.to_le_bytes()).collect(), None);
            view(p.normal.iter().flatten().flat_map(|f| f.to_le_bytes()).collect(), None);
        }
        // glTF keeps a short's largest value for restarting a strip.
        let short_indices = n <= usize::from(u16::MAX);
        if short_indices {
            view(p.index.iter().flat_map(|&i| (i as u16).to_le_bytes()).collect(), None);
        } else {
            view(p.index.iter().flat_map(|i| i.to_le_bytes()).collect(), None);
        }
        let (point_type, normal_type) = if packed { ("5122,\"normalized\":true", "5120,\"normalized\":true") } else { ("5126", "5126") };
        let _ = write!(
            accessors,
            "{sep}{{\"bufferView\":{a},\"componentType\":{point_type},\"count\":{n},\"type\":\"VEC3\",\"min\":[{},{},{}],\"max\":[{},{},{}]}},\
             {{\"bufferView\":{},\"componentType\":{normal_type},\"count\":{n},\"type\":\"VEC3\"}},\
             {{\"bufferView\":{},\"componentType\":{},\"count\":{},\"type\":\"SCALAR\"}}",
            num(lo[0]), num(lo[1]), num(lo[2]), num(hi[0]), num(hi[1]), num(hi[2]), a + 1, a + 2, if short_indices { 5123 } else { 5125 }, p.index.len()
        );
        let _ = write!(materials, "{sep}{}", material_json(&p.material));
        for name in extensions_of(&p.material) {
            if !used.contains(&name) {
                used.push(name);
            }
        }
        let _ = write!(primitives, "{sep}{{\"attributes\":{{\"POSITION\":{a},\"NORMAL\":{}}},\"indices\":{},\"material\":{i}}}", a + 1, a + 2);
    }
    // A name that would end the JSON string early loses what would.
    let generator: String = generator.chars().filter(|c| !matches!(c, '"' | '\\') && !c.is_control()).collect();
    let quoted = |names: &[&str]| names.iter().map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(",");
    let mut extensions = String::new();
    if !used.is_empty() {
        let _ = write!(extensions, "\"extensionsUsed\":[{}],", quoted(&used));
    }
    let node = if packed {
        let _ = write!(extensions, "\"extensionsRequired\":[{}],", quoted(&["KHR_mesh_quantization"]));
        format!("{{\"mesh\":0,\"translation\":[{},{},{}],\"scale\":[{r},{r},{r}]}}", num(middle[0]), num(middle[1]), num(middle[2]), r = num(reach))
    } else {
        "{\"mesh\":0}".to_string()
    };
    let json = format!(
        "{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"{generator}\"}},{extensions}\"scene\":0,\"scenes\":[{{\"nodes\":[0]}}],\"nodes\":[{node}],\
         \"meshes\":[{{\"primitives\":[{primitives}]}}],\"materials\":[{materials}],\"buffers\":[{{\"byteLength\":{}}}],\
         \"bufferViews\":[{views}],\"accessors\":[{accessors}]}}",
        bin.len()
    );
    let mut json = json.into_bytes();
    json.resize(json.len().div_ceil(4) * 4, b' ');
    let mut out = Vec::with_capacity(28 + json.len() + bin.len());
    out.extend(b"glTF");
    out.extend(2u32.to_le_bytes());
    out.extend(((12 + 8 + json.len() + 8 + bin.len()) as u32).to_le_bytes());
    out.extend((json.len() as u32).to_le_bytes());
    out.extend(b"JSON");
    out.extend(&json);
    out.extend((bin.len() as u32).to_le_bytes());
    out.extend(b"BIN\0");
    out.extend(&bin);
    out
}

/// What of a convex polygon lies inside the rectangle `w` and `h` either
/// side of the center (Sutherland and Hodgman's way, an edge at a time).
fn clip(mut poly: Vec<Corner>, w: f32, h: f32) -> Vec<Corner> {
    for (axis, sign, limit) in [(0, 1.0, w), (0, -1.0, w), (1, 1.0, h), (1, -1.0, h)] {
        let inside = |c: &Corner| sign * c.at[axis] <= limit;
        if poly.iter().all(inside) {
            continue;
        }
        let mut out = Vec::with_capacity(poly.len() + 2);
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            if inside(&a) {
                out.push(a);
            }
            if inside(&a) != inside(&b) {
                let t = (limit - sign * a.at[axis]) / (sign * (b.at[axis] - a.at[axis]));
                out.push(Corner { at: lerp(a.at, b.at, t), normal: lerp(a.normal, b.normal, t) });
            }
        }
        poly = out;
        if poly.len() < 3 {
            return Vec::new();
        }
    }
    poly
}

/// A camera in a world whose y is up, seeing it in perspective: it turns
/// the world's points into the picture's pixels, with a depth that grows as
/// one over the distance does, so it stays true when the terminal
/// interpolates it across a triangle.
pub struct Camera {
    eye: V3,
    right: V3,
    up: V3,
    ahead: V3,
    focal: f32,
    depth: f32,
}

impl Camera {
    /// Looking at `target` from `distance` away, turned `yaw` radians to
    /// the right of looking from in front (+z) and `pitch` above it, seeing
    /// `fov` radians up and down in a picture `height` pixels tall.
    pub fn orbit(target: V3, yaw: f32, pitch: f32, distance: f32, fov: f32, height: f32) -> Self {
        let eye = add(target, mul([yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos()], distance));
        let ahead = normalize(sub(target, eye));
        let right = normalize(cross(ahead, [0.0, 1.0, 0.0]));
        let up = cross(right, ahead);
        Self { eye, right, up, ahead, focal: height * 0.5 / (fov * 0.5).tan(), depth: height * 2.0 }
    }

    /// Where `p` shows, if it's in front of the eye.
    pub fn project(&self, p: V3) -> Option<V3> {
        let d = sub(p, self.eye);
        let z = dot(d, self.ahead);
        (z > 0.05).then(|| [self.focal * dot(d, self.right) / z, self.focal * dot(d, self.up) / z, self.depth / z])
    }

    /// A direction in the world as the picture has it: x right, y up, z
    /// toward the eye.
    pub fn turn(&self, n: V3) -> V3 {
        [dot(n, self.right), dot(n, self.up), -dot(n, self.ahead)]
    }

    /// A flat face of the world turned `normal` way, in a color or a
    /// [`Material`], if it faces the eye.
    pub fn face(&self, scene: &mut Scene, material: impl Into<Material>, points: &[V3], normal: V3) {
        let center = mul(points.iter().fold([0.0; 3], |a, &p| add(a, p)), 1.0 / points.len() as f32);
        if dot(sub(self.eye, center), normal) <= 0.0 {
            return;
        }
        let n = self.turn(normal);
        let corners: Option<Vec<Corner>> = points.iter().map(|&p| self.project(p).map(|at| Corner { at, normal: n })).collect();
        if let Some(corners) = corners {
            scene.polygon(material, &corners);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_polygon_past_the_edge_is_cut_to_the_picture() {
        let mut scene = Scene::new(100.0, 50.0);
        scene.flat(rgb(0xff0000), &[[-80.0, -10.0, 0.0], [20.0, -10.0, 0.0], [20.0, 10.0, 0.0], [-80.0, 10.0, 0.0]], [0.0, 0.0, 1.0]);
        let part = &scene.parts[0];
        assert!(part.pos.iter().all(|p| p[0] >= -50.0 - 1e-4), "nothing left of the edge: {:?}", part.pos);
        assert_eq!(scene.triangles(), 2);
        let mut outside = Scene::new(100.0, 50.0);
        outside.flat(rgb(0xff0000), &[[60.0, 0.0, 0.0], [70.0, 0.0, 0.0], [70.0, 5.0, 0.0]], [0.0, 0.0, 1.0]);
        assert_eq!(outside.triangles(), 0, "all outside, it's gone");
    }

    #[test]
    fn a_frame_reads_back_as_gltf_with_its_colors_and_the_pictures_corners() {
        let mut scene = Scene::new(200.0, 100.0);
        scene.flat(rgb(0xff0000), &[[0.0, 0.0, 5.0], [10.0, 0.0, 5.0], [10.0, 10.0, 5.0]], [0.0, 0.0, 1.0]);
        scene.flat([0.0, 0.0, 0.0, 0.4], &[[0.0, 0.0, -5.0], [-10.0, 0.0, -5.0], [-10.0, -10.0, -5.0]], [0.0, 0.0, 1.0]);
        let (doc, buffers, _) = gltf::import_slice(scene.glb((0.5, 1.0))).expect("a GLB gltf reads");
        let prims: Vec<_> = doc.meshes().flat_map(|m| m.primitives().collect::<Vec<_>>()).collect();
        assert_eq!(prims.len(), 3, "red, the shadow and the invisible corners");
        let colors: Vec<[f32; 4]> = prims.iter().map(|p| p.material().pbr_metallic_roughness().base_color_factor()).collect();
        assert_eq!(colors[0], [1.0, 0.0, 0.0, 1.0]);
        assert!((colors[1][3] - 0.4).abs() < 1e-6 && colors[2][3] == 0.0);
        let corners: Vec<[f32; 3]> = prims[2].reader(|b| Some(&buffers[b.index()].0)).read_positions().expect("positions").collect();
        assert_eq!(corners[0], [-50.0, -50.0, 0.0], "the corners are the picture's, squashed as the scene is");
        let red: Vec<[f32; 3]> = prims[0].reader(|b| Some(&buffers[b.index()].0)).read_positions().expect("positions").collect();
        assert_eq!(red[1], [5.0, 0.0, 5.0]);
    }

    #[test]
    fn a_face_lit_from_in_front_shows_the_color_asked_for() {
        let n = [0.0, 0.0, 1.0];
        let c = lit_as(rgb(0x8040c0), n);
        let shown = [c[0] * shade(n), c[1] * shade(n), c[2] * shade(n)];
        assert!((shown[0] - 0x80 as f32 / 255.0).abs() < 1e-4 && (shown[2] - 0xc0 as f32 / 255.0).abs() < 1e-4);
    }

    #[test]
    fn the_camera_sees_nearer_things_deeper_and_turns_away_from_the_back_of_a_face() {
        let cam = Camera::orbit([0.0; 3], 0.0, 0.0, 10.0, 0.6, 400.0);
        let near = cam.project([0.0, 0.0, 2.0]).expect("in front");
        let far = cam.project([0.0, 0.0, -2.0]).expect("in front");
        assert!(near[2] > far[2] && near[0].abs() < 1e-4);
        let right = cam.project([1.0, 0.0, 0.0]).expect("in front");
        assert!(right[0] > 0.0, "x is to the right");
        let mut scene = Scene::new(400.0, 400.0);
        let square = [[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0]];
        cam.face(&mut scene, rgb(0xffffff), &square, [0.0, 0.0, -1.0]);
        assert_eq!(scene.triangles(), 0, "its back is to the eye");
        cam.face(&mut scene, rgb(0xffffff), &square, [0.0, 0.0, 1.0]);
        assert_eq!(scene.triangles(), 2);
    }

    #[test]
    fn a_glb_names_the_program_that_made_it_and_its_json_stays_whole() {
        let scene = Scene::new(10.0, 10.0);
        let (doc, _, _) = gltf::import_slice(scene.glb_by((1.0, 1.0), "my \"game\"")).expect("it still reads");
        assert_eq!(doc.as_json().asset.generator.as_deref(), Some("my game"));
        let (doc, _, _) = gltf::import_slice(scene.glb((1.0, 1.0))).expect("a GLB gltf reads");
        assert_eq!(doc.as_json().asset.generator.as_deref(), Some("flow-sdk"));
    }

    /// A GLB's JSON chunk.
    fn json_of(glb: &[u8]) -> String {
        let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        String::from_utf8(glb[20..20 + len].to_vec()).unwrap()
    }

    #[test]
    fn a_plain_color_is_written_as_it_always_was_with_short_indices() {
        let mut scene = Scene::new(100.0, 100.0);
        scene.flat(rgb(0xff0000), &[[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]], [0.0, 0.0, 1.0]);
        scene.flat(Material::plain(rgb(0xff0000)), &[[0.0, 0.0, 0.0], [-10.0, 0.0, 0.0], [-10.0, -10.0, 0.0]], [0.0, 0.0, 1.0]);
        let json = json_of(&scene.glb((1.0, 1.0)));
        assert!(json.contains("\"materials\":[{\"pbrMetallicRoughness\":{\"baseColorFactor\":[1,0,0,1],\"metallicFactor\":0},\"alphaMode\":\"OPAQUE\",\"doubleSided\":true},"), "{json}");
        assert!(!json.contains("extensions") && !json.contains("roughness"), "nothing more said: {json}");
        assert_eq!(json.matches("\"pbrMetallicRoughness\"").count(), 2, "a color and its material are one part, and the corners");
        assert!(json.contains("\"componentType\":5123") && !json.contains("5125"), "indices in shorts");
    }

    #[test]
    fn a_material_reads_back_with_its_polish_metal_glow_and_coat() {
        let hull = Material::plain(rgb(0x101014)).gloss(0.85).metal(0.3).coat(0.7, 0.9);
        let lamp = Material::plain(rgb(0x202020)).glow([1.0, 0.5, 0.0], 3.0);
        let beam = Material::plain([1.0, 0.2, 0.2, 0.4]).unlit();
        assert!(!hull.is_plain() && Material::plain(rgb(0x101014)).is_plain() && Material::from(rgb(0x101014)).is_plain());
        let mut mesh = crate::mesh::Mesh::new();
        let square = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]];
        mesh.face(hull, &square, [0.5, 0.5, -1.0]);
        mesh.face(lamp, &square, [0.5, 0.5, -1.0]);
        mesh.face(beam, &square, [0.5, 0.5, -1.0]);
        let glb = mesh.glb();
        let json = json_of(&glb);
        assert!(json.contains("\"extensionsUsed\":[\"KHR_materials_clearcoat\",\"KHR_materials_emissive_strength\",\"KHR_materials_unlit\"]"), "{json}");
        let (doc, _, _) = gltf::import_slice(&glb).expect("a GLB gltf reads");
        let m: Vec<gltf::Material> = doc.materials().collect();
        let pbr = m[0].pbr_metallic_roughness();
        assert!((pbr.roughness_factor() - 0.15).abs() < 1e-6 && (pbr.metallic_factor() - 0.3).abs() < 1e-6);
        let coat = m[0].extension_value("KHR_materials_clearcoat").expect("a clear coat");
        assert_eq!((coat["clearcoatFactor"].as_f64(), coat["clearcoatRoughnessFactor"].as_f64().map(|r| (r * 1e6).round() / 1e6)), (Some(0.7), Some(0.1)));
        assert_eq!(m[1].emissive_factor()[0], 1.0);
        assert!((m[1].emissive_factor()[1] - 0.5f32.powf(2.2)).abs() < 1e-6, "linear light, as glTF has it");
        assert_eq!((m[1].emissive_strength(), m[1].unlit(), m[0].emissive_strength()), (Some(3.0), false, None));
        assert!(m[2].unlit() && m[2].alpha_mode() == gltf::material::AlphaMode::Blend);
    }

    #[test]
    fn indices_go_in_whole_words_only_past_a_shorts_reach() {
        let mut mesh = crate::mesh::Mesh::new();
        for i in 0..21_846 {
            let x = i as f32;
            mesh.triangle(rgb(0xffffff), [x, 0.0, 0.0], [x + 1.0, 0.0, 0.0], [x, 1.0, 0.0], [x, 0.0, -1.0]);
        }
        let json = json_of(&mesh.glb());
        assert!(json.contains("\"componentType\":5125,\"count\":65538"), "65,538 vertices are past a short");
        let (doc, buffers, _) = gltf::import_slice(mesh.glb()).expect("a GLB gltf reads");
        let prim = doc.meshes().next().unwrap().primitives().next().unwrap();
        let last = prim.reader(|b| Some(&buffers[b.index()].0)).read_indices().unwrap().into_u32().last();
        assert_eq!(last, Some(65_537));
    }

    /// A packed GLB's document, its required packing taken off the list as
    /// Flow does, and its numbers.
    fn packed(glb: &[u8]) -> (gltf::Document, Vec<u8>) {
        let gltf::Gltf { document, blob } = gltf::Gltf::from_slice_without_validation(glb).expect("a GLB");
        let mut json = document.into_json();
        assert_eq!(json.extensions_required, ["KHR_mesh_quantization"]);
        json.extensions_required.clear();
        (gltf::Document::from_json(json).expect("valid but for its packing"), blob.expect("its numbers"))
    }

    /// An accessor's whole numbers made -1 to 1 again.
    fn unpacked(blob: &[u8], accessor: &gltf::Accessor) -> Vec<V3> {
        let view = accessor.view().expect("a view");
        let stride = view.stride().expect("a stride");
        let at = view.offset() + accessor.offset();
        (0..accessor.count())
            .map(|i| {
                let p = at + i * stride;
                match accessor.data_type() {
                    gltf::accessor::DataType::I16 => [0, 1, 2].map(|k| f32::from(i16::from_le_bytes([blob[p + 2 * k], blob[p + 2 * k + 1]])) / 32767.0),
                    gltf::accessor::DataType::I8 => [0, 1, 2].map(|k| f32::from(blob[p + k] as i8) / 127.0),
                    other => panic!("{other:?} isn't packed"),
                }
            })
            .collect()
    }

    #[test]
    fn a_packed_scene_lands_where_it_was_facing_the_same_way_in_half_the_bytes() {
        let mut scene = Scene::new(1200.0, 700.0);
        let tilt = normalize([0.3, -0.5, 0.8]);
        scene.flat(Material::plain(rgb(0x101014)).gloss(0.9), &[[-300.0, -100.0, 4.0], [250.0, -90.0, 6.0], [10.0, 300.0, 5.0]], tilt);
        scene.flat(rgb(0xff8000), &[[100.0, 100.0, 1.0], [200.0, 100.0, 1.0], [200.0, 200.0, 1.0], [100.0, 200.0, 1.0]], [0.0, 0.0, 1.0]);
        // A field of tiles, for the numbers to outweigh the JSON.
        for i in 0..300 {
            let (x, y) = ((i % 20) as f32 * 20.0 - 400.0, (i / 20) as f32 * 20.0 - 300.0);
            scene.flat(rgb(0x2040ff), &[[x, y, 0.5], [x + 15.0, y, 0.5], [x + 15.0, y + 15.0, 0.5], [x, y + 15.0, 0.5]], [0.0, 0.0, 1.0]);
        }
        let (plain, small) = (scene.glb((0.9, 1.0)), scene.glb_packed((0.9, 1.0), "flow-sdk"));
        assert!(small.len() * 10 < plain.len() * 7, "{} bytes packed, {} not", small.len(), plain.len());
        let (doc, blob) = packed(&small);
        let (t, _, s) = doc.nodes().next().unwrap().transform().decomposed();
        assert!(s[0] == s[1] && s[1] == s[2], "one scale, so normals keep their way: {s:?}");
        let prims: Vec<_> = doc.meshes().next().unwrap().primitives().collect();
        assert_eq!(prims.len(), 4, "the hull, the square, the tiles and the corners");
        let points = unpacked(&blob, &prims[0].get(&gltf::Semantic::Positions).unwrap());
        let placed: Vec<V3> = points.iter().map(|q| [0, 1, 2].map(|k| t[k] + s[k] * q[k])).collect();
        for (got, want) in placed.iter().zip([[-270.0, -100.0, 4.0], [225.0, -90.0, 6.0], [9.0, 300.0, 5.0]]) {
            assert!((0..3).all(|k| (got[k] - want[k]).abs() < 0.02), "{got:?} for {want:?}");
        }
        let normals = unpacked(&blob, &prims[0].get(&gltf::Semantic::Normals).unwrap());
        let want = normalize([tilt[0] / 0.9, tilt[1], tilt[2]]);
        assert!(normals.iter().all(|n| dot(normalize(*n), want) > 0.9999), "{normals:?}");
        let corners = unpacked(&blob, &prims[3].get(&gltf::Semantic::Positions).unwrap());
        assert!((t[0] + s[0] * corners[2][0] - 540.0).abs() < 0.02 && (t[1] + s[1] * corners[2][1] - 350.0).abs() < 0.02, "the picture's corner");
        assert!(json_of(&small).contains("\"roughnessFactor\":"), "its materials as they'd be unpacked");
    }

    #[test]
    fn a_quarter_turn_about_y_takes_x_to_minus_z() {
        let v = apply(rotation([0.0, 1.0, 0.0], std::f32::consts::FRAC_PI_2), [1.0, 0.0, 0.0]);
        assert!((v[2] + 1.0).abs() < 1e-5 && v[0].abs() < 1e-5);
        let twice = compose(rotation([0.0, 1.0, 0.0], 0.3), rotation([0.0, 1.0, 0.0], 0.4));
        let once = rotation([0.0, 1.0, 0.0], 0.7);
        assert!((0..3).all(|i| (0..3).all(|j| (twice[i][j] - once[i][j]).abs() < 1e-5)));
    }
}
