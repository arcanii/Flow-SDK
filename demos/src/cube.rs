//! Cube: a small cube drops onto the top of a big cube of rounded blocks,
//! three by three by three with a slim gap between them, that's missing the
//! block at its top right, and rolls about it, each square it lands on
//! lighting up, until the squares spell an F. Then it tips over the edge
//! into the missing block and fits, and the big cube is whole: the light
//! runs from the top's F down the strokes of an F on every face, and the eye
//! pulls back and goes slowly once round it, after the GameCube's startup.
//! The landings ripple Flow's liquid with a thud, the reveal bursts it with
//! a chime, and the name comes up below a letter at a time. Space plays it
//! again.
//!
//! The blocks are violet glass, their far sides showing through their near
//! ones and the F's faces lit cyan from inside, the notch a hole in shadow
//! until it's filled; m, or 1 to 4, makes them polished anodised metal,
//! matte chipped stone whose F glows like embers through cracks in its
//! crust, or jelly that squashes, sways and wobbles where the small cube
//! lands, rolls and fits, all without missing a beat ([`Stuff`]). Solid
//! blocks close the gaps between them on a dark core, as a real cube's do.
//!
//! The scene goes to Flow as a new model every frame while it moves, and
//! Flow cuts a model's see-through triangles where they cross and sorts
//! them, farthest first, as it loads it, drawing them in runs of one
//! material. Its cutting takes longer the more triangles there are, and
//! longer still the more of them are small beside the rest. So a
//! see-through cube keeps them under 2,048 ([`SEE_THROUGH`]): a block's
//! rounds are cut as finely as its size on the screen asks, the finest
//! made coarser first until they fit ([`cut`]); its finely cut corners
//! are fans from their middles ([`fanned`]); the faces between blocks are
//! left out; the far sides are cut more coarsely than the near where those
//! are finely cut, from points of them, so that the two never cross; and
//! the F's light and the notch's shadow are solid, drawn before the glass.
//! And its near and far sides are one glass, the F's tinted faces the only
//! other see-through material, so that a frame sorts into a hundred runs at
//! most, well within the 256 Flow keeps to. Each block's triangles go as
//! fans, so that most corners serve several of them.

use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::hash::{BuildHasherDefault, Hasher};
use std::time::Duration;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use flow_sdk::fx::{Kind, Sound};
use flow_sdk::input::KeyCode;
use flow_sdk::scene::{add, apply, compose, cross, dot, lerp, mix, mul, normalize, rgb, rotation, shade, smooth, sub, Camera, Corner, Material, Scene, IDENTITY, M3, V3};
use flow_sdk::stage::Fx;

use crate::{Demo, Frame, Line, Show, FRAME};

/// The words' grey.
const WORDS: u32 = 0x9aa0b4;

/// The big cube's top is 3 × 3 squares; a square is (across, back to front).
type Square = (i32, i32);
/// Where the small cube drops, then where it rolls: the first six squares
/// it lands on light up (they spell an F, its top bar at the back), the
/// rest take it to the front right corner, where the big cube's block is
/// missing, so the last roll tips it over the edge and into its place.
const START: Square = (0, 2);
const PATH: [Square; 8] = [(0, 1), (0, 0), (1, 0), (2, 0), (1, 0), (1, 1), (2, 1), (2, 2)];
const LIGHTING: usize = 6;
/// The rolls along the top, before the one into the notch.
const ON_TOP: usize = PATH.len() - 1;
/// The missing block, and the small cube's middle once it's in.
const NOTCH: [i32; 3] = [2, 2, 2];
const IN_NOTCH: V3 = [2.5, 2.5, 2.5];
/// The big cube's middle.
const MIDDLE: V3 = [1.5, 1.5, 1.5];

/// When it starts to fall and lands, and the first roll.
const FALL: f32 = 0.3;
const LANDS: f32 = 0.85;
const ROLLS: f32 = 1.2;
/// A roll's seconds, and the pause after it.
const ROLL: f32 = 0.34;
const STEP: f32 = 0.42;
/// The half turn over the edge into the notch, a beat after the last roll
/// on the top, how long it takes, and the little hop it settles with.
const TIPS: f32 = ROLLS + ON_TOP as f32 * STEP + 0.12;
const TIP: f32 = 0.5;
const FITS: f32 = TIPS + TIP;
const SETTLE: f32 = 0.22;
/// The big cube bursts the liquid and the light sets off from the top's F
/// down the faces, a square further each `SPREAD`.
const REVEAL: f32 = FITS + 0.3;
const SPREAD: f32 = 0.13;
/// How long a square's light flashes as it comes on.
const FLASH: f32 = 0.3;
/// The eye pulls back between these, and goes once round the cube, slowly,
/// once the light has spread.
const PULL: (f32, f32) = (3.5, 5.4);
const TURN: (f32, f32) = (5.6, 11.4);
/// The name's letters come up, and it's all still.
const LETTERS: f32 = 10.8;
const DONE: f32 = 11.7;
/// How often it's drawn once it's still, but at once after a key: the
/// same frame again, which isn't sent.
const STILL: Duration = Duration::from_millis(100);
const NAME: [char; 4] = ['F', 'L', 'O', 'W'];

/// The big cube's faces: the way each looks out, and which way is up on it
/// seen from outside. The top's up is toward the back, so its F reads as
/// it does from the front; the bottom's is toward the front.
const FACES: [(V3, V3); 6] = [
    ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ([-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ([0.0, 1.0, 0.0], [0.0, 0.0, -1.0]),
    ([0.0, -1.0, 0.0], [0.0, 0.0, 1.0]),
    ([0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
    ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
];
const TOP: usize = 2;

/// The six ways out of a block, in the order a block's faces are counted:
/// +x, -x, +y, -y, +z, -z (the same as `FACES`').
const WAYS: [V3; 6] = [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]];

/// A square on the big cube's outside: its face, and where it is on it as
/// seen from outside, its column from the left and its row from the top.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tile {
    face: usize,
    col: i32,
    row: i32,
}

impl Tile {
    fn all() -> impl Iterator<Item = Tile> {
        (0..6).flat_map(|face| (0..9).map(move |i| Tile { face, col: i % 3, row: i / 3 }))
    }

    fn out(self) -> V3 {
        FACES[self.face].0
    }

    fn up(self) -> V3 {
        FACES[self.face].1
    }

    fn right(self) -> V3 {
        cross(mul(self.out(), -1.0), self.up())
    }

    fn middle(self) -> V3 {
        add(MIDDLE, add(mul(self.out(), 1.5), add(mul(self.right(), (self.col - 1) as f32), mul(self.up(), (1 - self.row) as f32))))
    }

    /// The tile at `middle` on the face looking `out`.
    fn at(middle: V3, out: V3) -> Tile {
        let face = FACES.iter().position(|f| dot(f.0, out) > 0.9).expect("a face looks every way");
        let probe = Tile { face, col: 1, row: 1 };
        let off = sub(middle, probe.middle());
        Tile { face, col: (dot(off, probe.right()) + 1.0).round() as i32, row: (1.0 - dot(off, probe.up())).round() as i32 }
    }

    /// Part of the F each face shows: its bar along the top, its stem down
    /// the left, and its arm.
    fn in_f(self) -> bool {
        self.row == 0 || self.col == 0 || (self.col, self.row) == (1, 1)
    }
}

/// What the cube is made of, which m (or 1 to 4) changes as it plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stuff {
    /// Violet glass, the far sides showing through the near ones, the F's
    /// faces lit cyan from inside.
    Glass,
    /// Polished metal anodised violet, the F's faces bright.
    Metal,
    /// Matte stone, chipped and a little different from block to block,
    /// the F glowing like embers.
    Rock,
    /// Bright see-through jelly that wobbles as it's struck.
    Jelly,
}

impl Stuff {
    const ALL: [Stuff; 4] = [Stuff::Glass, Stuff::Metal, Stuff::Rock, Stuff::Jelly];

    fn name(self) -> &'static str {
        match self {
            Stuff::Glass => "glass",
            Stuff::Metal => "metal",
            Stuff::Rock => "rock",
            Stuff::Jelly => "jelly",
        }
    }

    fn next(self) -> Stuff {
        let i = Self::ALL.iter().position(|&s| s == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    /// Block `n`'s shape: half its side (a little under half the space it
    /// has, for the gap between blocks) and how round its edges are.
    /// Stone's are chipped, some blocks' more than others'.
    fn shape(self, n: usize) -> (f32, f32) {
        match self {
            Stuff::Glass => (0.46, 0.14),
            Stuff::Metal => (0.465, 0.1),
            Stuff::Rock => (0.46, 0.05 + 0.035 * grain(n, 1)),
            Stuff::Jelly => (0.45, 0.2),
        }
    }

    /// The most steps a quarter round is cut in, close up (fewer far off,
    /// [`steps_for`]): four for glass, six for metal, whose highlights run
    /// round its edges, and for jelly, whose rounds are the largest (the
    /// see-through ones kept under 2,048 triangles a frame by [`cut`],
    /// however many these allow), and two for stone, whose edges are
    /// chipped rather than round ([`chip`]).
    fn most_steps(self) -> usize {
        match self {
            Stuff::Glass => 4,
            Stuff::Jelly => 6,
            Stuff::Metal => 6,
            Stuff::Rock => 2,
        }
    }

    /// How far its faces' normals lean out from their middles, as on a
    /// cushion: polished metal's, so that a highlight sweeps across each
    /// face as it turns, as it does on real metal, the F's faces' too.
    fn pillow(self) -> f32 {
        if self == Stuff::Metal {
            0.6
        } else {
            0.0
        }
    }

    /// Whether its far sides show through its near ones.
    fn see_through(self) -> bool {
        self.inner().is_some()
    }

    /// The color of its F's light: the burst's, and the name's below.
    fn light(self) -> u32 {
        match self {
            Stuff::Glass | Stuff::Metal => 0x7ce7ff,
            Stuff::Rock => 0xffa04a,
            Stuff::Jelly => 0x6dffc9,
        }
    }

    /// Whether its corners catch the light in sparkles as it turns.
    fn sparkles(self) -> bool {
        !matches!(self, Stuff::Rock)
    }

    /// A block's own surface, block `n` of the 27 (stone varies a little
    /// from block to block).
    fn body(self, n: usize) -> Material {
        match self {
            // Violet you can see into: polished and clear coated, the
            // highlights running round the rounded edges, with a little
            // light of its own so that its sides in shadow stay violet on
            // the dark glass. Each layer of it covers enough of what's
            // behind that the one nearest the eye stands out.
            Stuff::Glass => see_through(0x5230f0, 0.68).gloss(0.6).coat(0.25, 1.0).glow(own(0x5230f0), 0.12),
            // Anodised: all metal, polished, so that what it reflects takes
            // its violet, pale enough that its highlights whiten, with a
            // trace of light of its own so its sides that reflect the dark
            // below aren't black.
            Stuff::Metal => Material::plain(rgb(0x483a94)).metal(1.0).gloss(0.97).coat(0.3, 1.0).glow(own(0x483a94), 0.02),
            // A few shades of stone, slate to sandstone.
            Stuff::Rock => stone(1 + ((grain(n, 0) * 3.0) as usize).min(2), 0),
            // Bright and soft: less polished than glass, more of its own
            // light.
            Stuff::Jelly => see_through(0xb048ff, 0.55).gloss(0.45).glow(own(0xb048ff), 0.2),
        }
    }

    /// The small cube's own surface as it rolls, before it's one of the big
    /// cube's blocks: a paler cut of the same for the solid ones, and the
    /// same for the see-through, whose one see-through material Flow can
    /// sort in one run.
    fn small(self) -> Material {
        match self {
            Stuff::Metal => Material::plain(rgb(0x5c4cb0)).metal(1.0).gloss(0.95).coat(0.3, 1.0).glow(own(0x5c4cb0), 0.02),
            Stuff::Rock => stone(4, 0),
            Stuff::Glass | Stuff::Jelly => self.body(0),
        }
    }

    /// The inside of a see-through block's far side, seen through its near
    /// one: the same glass, so that Flow, which draws see-through triangles
    /// farthest first in runs of one material, sorts them in one run.
    fn inner(self) -> Option<Material> {
        match self {
            Stuff::Glass | Stuff::Jelly => Some(self.body(0)),
            Stuff::Metal | Stuff::Rock => None,
        }
    }

    /// The see-through stuff's walls round the notch until it's filled, or
    /// its `floor`, and the rounded edges and corners round them: in
    /// shadow, dark and dull, the floor darker, so the notch reads as a
    /// hole among the layers seen through the top round it, not as a lid.
    /// Solid, which nothing shows through in a shadow that deep anyway:
    /// Flow sorts see-through triangles in runs of one material, and these,
    /// among the glass's, would break it into more runs than it keeps to.
    /// (Solid stuff shows the notch plainly.)
    fn notch(self, floor: bool) -> Option<Material> {
        let (wall, glow) = match self {
            Stuff::Glass => (0x1c0e4c, 0x1e1060),
            Stuff::Jelly => (0x360c50, 0x40105c),
            Stuff::Metal | Stuff::Rock => return None,
        };
        let k = if floor { 0.55 } else { 1.0 };
        let c = rgb(wall);
        Some(Material::plain([c[0] * k, c[1] * k, c[2] * k, 1.0]).gloss(0.25).glow(own(glow), 0.1 * k))
    }

    /// What closes the gaps between solid blocks, as a real cube's core
    /// does: dark and matte, so that the gaps read as grooves.
    fn core(self) -> Option<Material> {
        match self {
            Stuff::Metal => Some(Material::plain(rgb(0x0c0818))),
            Stuff::Rock => Some(Material::plain(rgb(0x0e0b0a))),
            Stuff::Glass | Stuff::Jelly => None,
        }
    }

    /// The see-through face over a lit block's light, tinted with it, for
    /// the see-through stuff (its light is under it, [`Stuff::lit`]):
    /// enough of its own light that between the light and its edges it
    /// reads as tinted light, not as the grey of what's behind. Jelly's is
    /// less polished, so the light falling on its top doesn't pale it, and
    /// covers more, since mint over the magenta behind it would grey.
    fn lit_cover(self) -> Option<Material> {
        match self {
            Stuff::Glass => Some(see_through(0x58d8ff, 0.42).gloss(0.85).coat(0.35, 1.0).glow(own(0x58d8ff), 0.45)),
            Stuff::Jelly => Some(see_through(0x38f090, 0.6).gloss(0.2).glow(own(0x38f090), 0.5)),
            Stuff::Metal | Stuff::Rock => None,
        }
    }

    /// How far from a lit see-through face's middle its light's halo and
    /// heart reach, and how far under its face (the lowest it sinks) each
    /// sits, for a block shaped as `form` is. Glass's halo sits deep and
    /// reaches under the rounds, so the light shifts under the face as the
    /// cube turns. Jelly's sit just under the face and keep to its flat
    /// part: seen at a slant, a line of sight through the face must meet
    /// the light, not the magenta behind it, which with the mint over it
    /// would grey.
    fn light_layers(self, form: &Form) -> [(f32, f32); 2] {
        let inner = form.half - form.round;
        match self {
            Stuff::Jelly => [(inner, 0.03), (0.7 * inner, 0.015)],
            _ => {
                let reach = inner + 0.3 * form.round;
                [(reach, 0.13), (0.6 * reach, 0.06)]
            }
        }
    }

    /// A face of the F once it's lit: brighter and paler by `flash`, from 0
    /// to 1, as it comes on but never white, and less of its own light as
    /// far as the face is `in_light` (0 to 1), so that the light falling on
    /// it and its own don't add up to white. Solid, so a see-through cube's
    /// glass blends over it.
    fn lit(self, flash: f32, in_light: f32) -> Glowing {
        let flash = (flash * 8.0).round() / 8.0;
        // The see-through stuff's light shows through its tinted face,
        // which takes the light falling on it, and stone's is matte, which
        // never reaches white.
        let in_light = if self.see_through() || self == Stuff::Rock { 0.0 } else { in_light };
        let glowing = |under: u32, glow: u32, strength: f32| {
            let g = mix(rgb(glow), rgb(0xffffff), flash * 0.35);
            Material::plain(rgb(under)).glow([g[0], g[1], g[2]], strength * (1.0 - 0.4 * in_light) + 0.3 * flash)
        };
        match self {
            // A cyan heart in a dimmer halo, under the glass.
            Stuff::Glass => Glowing { heart: glowing(0x062c3c, 0x46d8ff, 1.0), halo: Some(glowing(0x041c2a, 0x1494d0, 0.6)), crust: None, ash: None },
            // Polished cyan, all metal so that what it reflects stays cyan,
            // the room's bright ceiling and dark floor and the light's
            // highlights crossing it as it turns, with a little light of
            // its own, a little more turned from the light, so that its
            // faces reflecting the dark below still read as lit.
            Stuff::Metal => {
                let g = mix(rgb(0x3cc8f0), rgb(0xffffff), flash * 0.35);
                Glowing { heart: Material::plain(rgb(0x0c5a78)).metal(1.0).gloss(0.95).coat(0.3, 1.0).glow([g[0], g[1], g[2]], 0.02 + 0.25 * (1.0 - in_light) + 0.3 * flash), halo: None, crust: None, ash: None }
            }
            // Embers ([`Brush::embers`]): a hot core along each crack
            // (which flashes), orange, and deep red, which warms the edges
            // of the plates of crust it parts, their middles dark ash as
            // matte as the stone round them.
            Stuff::Rock => Glowing {
                heart: glowing(0x6a3a08, 0xffd040, 1.0).gloss(0.1),
                halo: Some(glowing(0x3a0c02, 0xc82a0a, 0.8).gloss(0.1)),
                crust: Some(Material::plain(rgb(0x1e100c)).gloss(0.1).glow(own(0x5a1406), 0.3)),
                ash: Some(Material::plain(rgb(0x34302e)).gloss(0.12).glow(own(0x1e0c06), 0.1)),
            },
            // A mint heart in a dimmer halo, under the jelly.
            Stuff::Jelly => Glowing { heart: glowing(0x063c20, 0x34ff94, 1.0), halo: Some(glowing(0x042814, 0x1ad874, 0.75)), crust: None, ash: None },
        }
    }
}

/// A lit face's materials: its light's heart, and a dimmer glow round it
/// (the see-through stuff's halo, sunk deeper); for stone, the hot core of
/// its cracks and the deep red round it, and the plates of crust they
/// part, their edges warmed by it and their middles dark ash.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Glowing {
    heart: Material,
    halo: Option<Material>,
    crust: Option<Material>,
    ash: Option<Material>,
}

impl Glowing {
    #[cfg(test)]
    fn all(&self) -> impl Iterator<Item = Material> {
        [Some(self.heart), self.halo, self.crust, self.ash].into_iter().flatten()
    }
}

/// A color as a glow, without its alpha.
fn own(hex: u32) -> [f32; 3] {
    let c = rgb(hex);
    [c[0], c[1], c[2]]
}

/// A see-through color, `alpha` of it covering what's behind.
fn see_through(hex: u32, alpha: f32) -> Material {
    let c = rgb(hex);
    Material::plain([c[0], c[1], c[2], alpha])
}

/// Stone at `level` (0 to 4) from slate to sandstone, a fleck's `tone` a
/// shade lighter or darker: matte, the shaded sides lifted a little off the
/// dark.
fn stone(level: usize, tone: i8) -> Material {
    let level = (level as i32 + i32::from(tone)).clamp(0, 4) as f32;
    let c = mix(rgb(0x40405a), rgb(0x70604a), level / 4.0);
    Material::plain(c).gloss(0.12).glow([c[0], c[1], c[2]], 0.12)
}

/// A fleck of `own`, a shade lighter (`tone` 1) or darker (-1), and its
/// glow with it.
fn fleck(own: Material, tone: i8) -> Material {
    let k = 1.0 + 0.12 * f32::from(tone);
    let c = own.color;
    Material { color: [(c[0] * k).min(1.0), (c[1] * k).min(1.0), (c[2] * k).min(1.0), c[3]], glow_strength: own.glow_strength * k, ..own }
}

/// A number from 0 to 1 that's the same for `n` and `k` each time and
/// differs from its neighbours': for stone's variety, block to block and
/// face to face.
fn grain(n: usize, k: usize) -> f32 {
    let mut x = (n as u32).wrapping_mul(0x9e37_79b9) ^ (k as u32).wrapping_mul(0x85eb_ca6b).wrapping_add(0x27d4_eb2f);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297a_2d39);
    x ^= x >> 15;
    (x & 0xffff) as f32 / 65535.0
}

/// How many steps a quarter round `pixels` across on the screen is cut in,
/// at most `most`: enough that its straight cuts stay within a pixel or so
/// of the round, which takes more close up than far off.
fn steps_for(pixels: f32, most: usize) -> usize {
    ((pixels * 0.45).sqrt().ceil() as usize).clamp(2, most.max(2))
}

/// How many steps a see-through block's far side, seen only through its
/// near side, is cut in when its near side is cut in `near`: half as many
/// where that's two or more, and so that each of its points is one of the
/// near side's, which keeps it inside the near side (two cuts that cross
/// each other, Flow would cut where they cross, into more triangles); and
/// as many where it's fewer, since a far side cut more coarsely still
/// shows through as a smaller block inside the round one.
fn far_steps(near: usize) -> usize {
    if near.is_multiple_of(2) && near >= 4 {
        near / 2
    } else {
        near
    }
}

/// How a block's rounded surface is cut and shaded, in its own frame (its
/// middle at the origin, its sides along the axes): half its side, how
/// round its edges are, and in how many steps a quarter round is cut; how
/// far its normals lean out from each face's middle (`pillow`), and on the
/// face that looks `up` by `dome` more, with that face's middle raised by
/// `rise`, as jelly's is as it wobbles; stone's facets, seeded by the
/// block's number; and for a far side, the steps its near side is cut in,
/// whose corners it's cut as ([`fanned`]).
#[derive(Debug, Clone, Copy)]
struct Form {
    half: f32,
    round: f32,
    steps: usize,
    pillow: f32,
    up: V3,
    dome: f32,
    rise: f32,
    facets: Option<usize>,
    near: Option<usize>,
}

impl Form {
    fn new(half: f32, round: f32, steps: usize) -> Self {
        Self { half, round, steps, pillow: 0.0, up: [0.0, 1.0, 0.0], dome: 0.0, rise: 0.0, facets: None, near: None }
    }

    /// The way the surface at `p`, whose true normal is `n`, is shaded:
    /// leaning out as far from its face's middle as `p` is.
    fn normal(&self, p: V3, n: V3) -> V3 {
        let lean = self.pillow + self.dome * dot(n, self.up).max(0.0).powi(2);
        if lean == 0.0 {
            return n;
        }
        let across = sub(p, mul(n, dot(p, n)));
        normalize(add(n, mul(across, lean / (self.half - self.round))))
    }
}

/// A rounded block's surface as triangles over shared corners.
#[derive(Debug, Default)]
struct Shell {
    at: Vec<V3>,
    normal: Vec<V3>,
    tris: Vec<Tri>,
    /// Each corner's number by where it is and which way it faces, so that
    /// the pieces share their corners.
    found: HashMap<[i32; 6], u32, BuildHasherDefault<Quick>>,
}

/// A quick hash for a shell's corners, found anew for every block each
/// frame: std's own guards against keys chosen to collide, which these,
/// worked out here, aren't, and takes several times as long.
#[derive(Debug, Default)]
struct Quick(u64);

impl Hasher for Quick {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for word in bytes.chunks(8) {
            let mut eight = [0u8; 8];
            eight[..word.len()].copy_from_slice(word);
            self.0 = (self.0.rotate_left(5) ^ u64::from_le_bytes(eight)).wrapping_mul(0x517c_c1b7_2722_0a95);
        }
    }
}

/// One of a shell's triangles: its corners, counter-clockwise seen from
/// outside; the flat face it's on, if it's on one; the faces it lies on or
/// beside, a bit each; and for stone, a fleck a shade lighter or darker.
#[derive(Debug, Clone, Copy)]
struct Tri {
    v: [u32; 3],
    face: Option<usize>,
    beside: u8,
    tone: i8,
}

impl Shell {
    fn corner(&mut self, at: V3, normal: V3) -> u32 {
        let key = [at[0], at[1], at[2], normal[0], normal[1], normal[2]].map(|c| (c * 1e5).round() as i32);
        if let Some(&i) = self.found.get(&key) {
            return i;
        }
        let i = self.at.len() as u32;
        self.at.push(at);
        self.normal.push(normal);
        self.found.insert(key, i);
        i
    }

    /// A triangle, wound to face the way its corners do.
    fn tri(&mut self, v: [u32; 3], face: Option<usize>, beside: u8, tone: i8) {
        let [a, b, c] = v.map(|i| self.at[i as usize]);
        let facing = v.iter().fold([0.0; 3], |s, &i| add(s, self.normal[i as usize]));
        let v = if dot(cross(sub(b, a), sub(c, a)), facing) >= 0.0 { v } else { [v[0], v[2], v[1]] };
        self.tris.push(Tri { v, face, beside, tone });
    }
}

/// A face's corners, along its sides (`u` and `v` in [`shell`]),
/// counter-clockwise seen from outside.
const SQUARE: [(f32, f32); 4] = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];

/// A rounded block's surface where it's `open`, by face (`WAYS`' order):
/// those faces, and the rounded edges and corners beside them. The rest
/// lies against another block, where no one sees it. Stone's is chipped
/// ([`chip`]).
fn shell(form: &Form, open: [bool; 6]) -> Shell {
    let (half, round, n) = (form.half, form.round, form.steps.max(1));
    let inner = half - round;
    let mut s = Shell::default();
    // A point of the surface: the inner box's corner or edge that's
    // `toward` (a sum of the axes), and `round` out from it `way`.
    let point = |toward: V3, way: V3| add(mul(toward, inner), mul(way, round));
    for f in (0..6).filter(|&f| open[f]) {
        let out = WAYS[f];
        let (u, v) = (along(out), cross(out, along(out)));
        if form.facets.is_some() {
            // Stone: the face one flat facet, as far as its edges are
            // chipped into it ([`chipped`]), its sides cut where its edges
            // are, so that they're chipped along their length too.
            let ring: Vec<u32> = (0..4)
                .flat_map(|i| (0..ALONG).map(move |j| (i, j as f32 / ALONG as f32)))
                .map(|(i, k)| {
                    let ((a0, b0), (a1, b1)) = (SQUARE[i], SQUARE[(i + 1) % 4]);
                    let p = point(add(out, add(mul(u, a0 + (a1 - a0) * k), mul(v, b0 + (b1 - b0) * k))), out);
                    s.corner(p, out)
                })
                .collect();
            let middle = s.corner(point(out, out), out);
            for k in 0..ring.len() {
                s.tri([middle, ring[k], ring[(k + 1) % ring.len()]], Some(f), 1 << f, 0);
            }
            continue;
        }
        let c = SQUARE.map(|(a, b)| point(add(out, add(mul(u, a), mul(v, b))), out)).map(|p| s.corner(p, form.normal(p, out)));
        if form.rise != 0.0 && dot(out, form.up) > 0.9 {
            // Its middle raised (or sunk), a fan of four about it.
            let m = s.corner(mul(out, half + form.rise), out);
            for k in 0..4 {
                s.tri([m, c[k], c[(k + 1) % 4]], Some(f), 1 << f, 0);
            }
        } else {
            s.tri([c[0], c[1], c[2]], Some(f), 1 << f, 0);
            s.tri([c[0], c[2], c[3]], Some(f), 1 << f, 0);
        }
    }
    // The edges between two faces at right angles, a quarter round from
    // one to the other in `n` steps, each step's diagonal turned the other
    // way from the last, so that four triangles meet at every other corner
    // along the round: a fan of them goes as six corners.
    for f1 in 0..6 {
        for f2 in f1 + 1..6 {
            if f1 / 2 == f2 / 2 || !(open[f1] || open[f2]) {
                continue;
            }
            let (d1, d2) = (WAYS[f1], WAYS[f2]);
            let axis = cross(d1, d2);
            // Each step's points along the edge: its ends, and for stone,
            // where its faces' sides are cut.
            let lengthwise = if form.facets.is_some() { ALONG } else { 1 };
            let ring: Vec<Vec<u32>> = (0..=n)
                .map(|k| {
                    let way = round_way(d1, d2, [0.0; 3], [n - k, k, 0]);
                    (0..=lengthwise)
                        .map(|j| {
                            let p = point(add(add(d1, d2), mul(axis, 2.0 * j as f32 / lengthwise as f32 - 1.0)), way);
                            s.corner(p, form.normal(p, way))
                        })
                        .collect()
                })
                .collect();
            let beside = 1 << f1 | 1 << f2;
            for k in 0..n {
                for j in 0..lengthwise {
                    let ([a0, b0], [a1, b1]) = ([ring[k][j], ring[k][j + 1]], [ring[k + 1][j], ring[k + 1][j + 1]]);
                    if (k + j) % 2 == 0 {
                        s.tri([a0, a1, b0], None, beside, 0);
                        s.tri([a1, b1, b0], None, beside, 0);
                    } else {
                        s.tri([a0, a1, b1], None, beside, 0);
                        s.tri([a0, b1, b0], None, beside, 0);
                    }
                }
            }
        }
    }
    // The corners, an eighth of a ball each, cut to meet the edges' steps.
    for signs in 0..8 {
        let sign = |bit: usize| if signs >> bit & 1 == 0 { 1.0 } else { -1.0 };
        let e = [[sign(0), 0.0, 0.0], [0.0, sign(1), 0.0], [0.0, 0.0, sign(2)]];
        let faces = [signs & 1, 2 + (signs >> 1 & 1), 4 + (signs >> 2 & 1)];
        if !faces.iter().any(|&f| open[f]) {
            continue;
        }
        let beside = faces.iter().fold(0, |b, &f| b | 1 << f);
        let toward = add(e[0], add(e[1], e[2]));
        let fan = fanned(form.near.unwrap_or(n));
        // Its points, by how many steps they are toward each of its ends:
        // a fan's round its edges only.
        let mut grid = HashMap::new();
        for a in 0..=n {
            for b in (0..=n - a).filter(|&b| !fan || a == 0 || b == 0 || a + b == n) {
                let way = round_way(e[0], e[1], e[2], [a, b, n - a - b]);
                let p = point(toward, way);
                grid.insert((a, b), s.corner(p, form.normal(p, way)));
            }
        }
        if fan {
            // A fan from its middle to its edges, cut as finely as its
            // rounds so that it meets them and keeps the faces' corners
            // round ([`fanned`]).
            let way = normalize(toward);
            let p = point(toward, way);
            let hub = s.corner(p, form.normal(p, way));
            let edge: Vec<u32> = (0..n).map(|k| grid[&(n - k, k)]).chain((0..n).map(|k| grid[&(0, n - k)])).chain((0..n).map(|k| grid[&(k, 0)])).collect();
            for k in 0..edge.len() {
                s.tri([hub, edge[k], edge[(k + 1) % edge.len()]], None, beside, 0);
            }
            continue;
        }
        for a in 0..n {
            for b in 0..n - a {
                s.tri([grid[&(a + 1, b)], grid[&(a, b + 1)], grid[&(a, b)]], None, beside, 0);
                if a + b + 2 <= n {
                    s.tri([grid[&(a + 1, b)], grid[&(a + 1, b + 1)], grid[&(a, b + 1)]], None, beside, 0);
                }
            }
        }
    }
    match form.facets {
        Some(block) => chip(&s, form, block),
        None => s,
    }
}

/// How many pieces a stone block's edges are cut in along their length,
/// each end of each chipped its own way.
const ALONG: usize = 2;
/// How far a stone face's sides are knocked in toward its middle, at the
/// most, and its rounds' middles sunk toward their edges, as fractions of
/// its round; and how far its face leans, at the most, at its sides.
const KNOCK: f32 = 1.2;
const SINK: f32 = 0.55;
const TILT: f32 = 0.022;

/// How far stone block `block`'s face `f` stands out at `a`, `b` across
/// it (-1 to 1 along its sides, as [`shell`] lays them): each face leans
/// a little its own way, as hewn stone's do.
fn tilt(block: usize, f: usize, a: f32, b: f32) -> f32 {
    let g = |k: usize| 2.0 * grain(block * 6 + f, k) - 1.0;
    TILT * 0.5 * (g(7) * a + g(8) * b)
}

/// Where stone block `block`'s point `p` of its rounded surface ends up
/// once chipped: a face's side knocked in toward its middle, a little or
/// at a few places a lot, a round's middle sunk toward its edge, and each
/// face leaning its own way ([`tilt`]). Each point by where it is, so that
/// the pieces of the surface that share it still meet, and each edge is
/// chipped otherwise along its length.
fn chipped(form: &Form, block: usize, p: V3) -> V3 {
    let inner = form.half - form.round;
    let c = p.map(|x| x.clamp(-inner, inner));
    let d = sub(p, c);
    let len = dot(d, d).sqrt();
    if len < 1e-6 {
        return p;
    }
    let way = mul(d, 1.0 / len);
    let key = p.map(|x| (x * 1e4).round() as i32 as u32);
    let at = key[0].wrapping_mul(73_856_093) ^ key[1].wrapping_mul(19_349_663) ^ key[2].wrapping_mul(83_492_791);
    let g = |k: usize| grain(block.wrapping_mul(1_000_003) ^ at as usize, k);
    let Some(k) = (0..3).find(|&k| way[k].abs() > 0.999) else { return sub(p, mul(way, form.round * SINK * g(4))) };
    let f = 2 * k + usize::from(way[k] < 0.0);
    let mut q = p;
    for j in (0..3).filter(|&j| j != k && (c[j].abs() - inner).abs() < 1e-4) {
        q[j] -= c[j].signum() * form.round * KNOCK * g(j).powi(2);
    }
    let (u, v) = (along(way), cross(way, along(way)));
    add(q, mul(way, tilt(block, f, dot(q, u) / inner, dot(q, v) / inner)))
}

/// Stone block `block`'s surface `s` chipped ([`chipped`]) and shaded
/// flat, each triangle facing the way it lies, so that every chip catches
/// the light its own way; each face a shade of its own, and some chips a
/// shade lighter, freshly broken, or darker.
fn chip(s: &Shell, form: &Form, block: usize) -> Shell {
    let at: Vec<V3> = s.at.iter().map(|&p| chipped(form, block, p)).collect();
    let mut out = Shell::default();
    for t in &s.tris {
        let [a, b, c] = t.v.map(|i| at[i as usize]);
        let mut flat = normalize(cross(sub(b, a), sub(c, a)));
        let was = t.v.iter().fold([0.0; 3], |n, &i| add(n, s.normal[i as usize]));
        if dot(flat, was) < 0.0 {
            flat = mul(flat, -1.0);
        }
        let shade = match t.face {
            Some(f) => grain(block * 6 + f, 9),
            None => {
                let m = t.v.iter().fold([0.0; 3], |m, &i| add(m, s.at[i as usize])).map(|x| (x * 1e3).round() as i32 as u32);
                grain(block ^ (m[0].wrapping_mul(2_654_435_761) ^ m[1].wrapping_mul(40_503) ^ m[2].wrapping_mul(97_331)) as usize, 5)
            }
        };
        let tone = match shade {
            x if x < 0.15 => -1,
            x if x > 0.7 => 1,
            _ => 0,
        };
        let v = [a, b, c].map(|p| out.corner(p, flat));
        out.tri(v, t.face, t.beside, tone);
    }
    out
}

/// Stone block `block`'s face `f`'s flat part's edge as it's chipped
/// ([`chipped`]), as [`shell`] has it: its points along its sides,
/// counter-clockwise seen from outside, in its plane along its sides, its
/// flat part's half side 1.
fn face_edge(form: &Form, block: usize, f: usize) -> Vec<[f32; 2]> {
    let out = WAYS[f];
    let (u, v) = (along(out), cross(out, along(out)));
    let inner = form.half - form.round;
    (0..4)
        .flat_map(|i| (0..ALONG).map(move |j| (i, j as f32 / ALONG as f32)))
        .map(|(i, k)| {
            let ((a0, b0), (a1, b1)) = (SQUARE[i], SQUARE[(i + 1) % 4]);
            let p = add(mul(add(out, add(mul(u, a0 + (a1 - a0) * k), mul(v, b0 + (b1 - b0) * k))), inner), mul(out, form.round));
            let c = chipped(form, block, p);
            [dot(c, u) / inner, dot(c, v) / inner]
        })
        .collect()
}

/// Whether a rounded block's corner cut in `steps` is a fan of triangles
/// from its middle to the points round its edges, rather than cut all over:
/// where it's finely cut, which takes fewer triangles, and larger ones.
/// Many small ones slow Flow's cutting of see-through triangles where they
/// cross, as it loads each frame. (A far side cut more coarsely than its
/// near side is cut as it is, so it stays inside it: its points are some
/// of the near side's.)
fn fanned(steps: usize) -> bool {
    steps >= 3
}

/// The way out from a rounded edge or corner `steps` of the way toward
/// each of `a`, `b` and `c`: exactly one of them at its end, so that an
/// edge's points and its corners' and faces' are the same.
fn round_way(a: V3, b: V3, c: V3, [i, j, k]: [usize; 3]) -> V3 {
    match [i, j, k] {
        [_, 0, 0] => a,
        [0, _, 0] => b,
        [0, 0, _] => c,
        _ => normalize(add(mul(a, i as f32), add(mul(b, j as f32), mul(c, k as f32)))),
    }
}

/// `tris`, whose corners are numbered below `corners`, as polygons each of
/// which is a fan of them from its first corner: a polygon of k corners
/// makes k - 2 triangles, so a fan of many takes not much more than a
/// corner a triangle, rather than three.
fn fans(tris: &[[u32; 3]], corners: usize) -> Vec<Vec<u32>> {
    let mut around: Vec<Vec<usize>> = vec![Vec::new(); corners];
    for (i, t) in tris.iter().enumerate() {
        for &c in t {
            around[c as usize].push(i);
        }
    }
    let mut left = vec![true; tris.len()];
    // How many of the triangles left meet at each corner, and the corners
    // by that, most first (the first numbered first among as many), kept
    // as triangles are taken: a stone block's hundred and more lone
    // triangles would each look through every corner otherwise.
    let mut count: Vec<usize> = around.iter().map(Vec::len).collect();
    let mut by_count: BTreeSet<(Reverse<usize>, usize)> = count.iter().enumerate().filter(|(_, &k)| k > 0).map(|(c, &k)| (Reverse(k), c)).collect();
    let take = |t: usize, left: &mut [bool], count: &mut [usize], by_count: &mut BTreeSet<(Reverse<usize>, usize)>| {
        left[t] = false;
        for &c in &tris[t] {
            let c = c as usize;
            by_count.remove(&(Reverse(count[c]), c));
            count[c] -= 1;
            if count[c] > 0 {
                by_count.insert((Reverse(count[c]), c));
            }
        }
    };
    let mut out = Vec::new();
    // The corner most of the triangles left meet at, while any are left.
    while let Some(&(_, hub)) = by_count.first() {
        // Its triangles, each as the side across from it, joined end to
        // end round it.
        let mut sides: Vec<(u32, u32, usize)> = around[hub]
            .iter()
            .filter(|&&t| left[t])
            .map(|&t| {
                let v = tris[t];
                let k = v.iter().position(|&c| c as usize == hub).expect("a corner of it");
                (v[(k + 1) % 3], v[(k + 2) % 3], t)
            })
            .collect();
        while !sides.is_empty() {
            let start = (0..sides.len()).find(|&i| !sides.iter().any(|s| s.1 == sides[i].0)).unwrap_or(0);
            let (a, b, t) = sides.remove(start);
            take(t, &mut left, &mut count, &mut by_count);
            let mut fan = vec![hub as u32, a, b];
            while let Some(i) = sides.iter().position(|s| Some(&s.0) == fan.last()) {
                let (_, b, t) = sides.remove(i);
                take(t, &mut left, &mut count, &mut by_count);
                fan.push(b);
            }
            out.push(fan);
        }
    }
    out
}

/// Points to a corner of a see-through light's [`rounded_square`]s:
/// enough that close up, where its heart is the brightest thing on the
/// screen, their corners read as round. (They're solid, so they cost no
/// see-through triangles.)
const ROUNDED: usize = 6;

/// A rounded square `reach` from its middle to its sides, its corners
/// rounded `round` of the way in: `points` to a corner, counter-clockwise.
fn rounded_square(reach: f32, round: f32, points: usize) -> Vec<[f32; 2]> {
    let r = reach * round;
    let c = reach - r;
    (0..4)
        .flat_map(|q| {
            let (sa, sb) = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)][q];
            (0..points).map(move |k| {
                let angle = (q as f32 + k as f32 / (points - 1) as f32) * FRAC_PI_2;
                [sa * c + r * angle.cos(), sb * c + r * angle.sin()]
            })
        })
        .collect()
}

/// `poly` cut to the side of the line halfway between `from` and `to`
/// that `from` is on, `gap` short of it.
fn nearer(poly: &[[f32; 2]], from: [f32; 2], to: [f32; 2], gap: f32) -> Vec<[f32; 2]> {
    let d = [to[0] - from[0], to[1] - from[1]];
    let len = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-6);
    let d = [d[0] / len, d[1] / len];
    let mid = [(from[0] + to[0]) * 0.5, (from[1] + to[1]) * 0.5];
    let past = |p: [f32; 2]| (p[0] - mid[0]) * d[0] + (p[1] - mid[1]) * d[1] + gap;
    let mut out = Vec::new();
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (pa, pb) = (past(a), past(b));
        if pa <= 0.0 {
            out.push(a);
        }
        if (pa <= 0.0) != (pb <= 0.0) {
            let t = pa / (pa - pb);
            out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        }
    }
    out
}

/// `poly` cut to within `convex`, whose corners go counter-clockwise.
fn within(poly: &[[f32; 2]], convex: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let mut out = poly.to_vec();
    for i in 0..convex.len() {
        let (a, b) = (convex[i], convex[(i + 1) % convex.len()]);
        let side = |p: [f32; 2]| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        let mut cut = Vec::new();
        for k in 0..out.len() {
            let (p, q) = (out[k], out[(k + 1) % out.len()]);
            let (sp, sq) = (side(p), side(q));
            if sp >= 0.0 {
                cut.push(p);
            }
            if (sp >= 0.0) != (sq >= 0.0) {
                let t = sp / (sp - sq);
                cut.push([p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]);
            }
        }
        out = cut;
    }
    out
}

/// Where a block is: an affine map from its own frame to the world, its
/// turn and its middle and, as jelly gives, its squash and lean.
#[derive(Debug, Clone, Copy)]
struct Placed {
    m: M3,
    at: V3,
}

impl Placed {
    fn still(middle: V3) -> Self {
        Self { m: IDENTITY, at: middle }
    }

    /// Then mapped by `a` and moved by `b`.
    fn then(self, a: M3, b: V3) -> Self {
        Self { m: compose(a, self.m), at: add(apply(a, self.at), b) }
    }

    fn point(&self, p: V3) -> V3 {
        add(apply(self.m, p), self.at)
    }

    /// The way a surface facing `n` in the block's frame faces in the
    /// world: by the map's cofactors, its inverse turned over and scaled.
    fn normal(&self, n: V3) -> V3 {
        let [a, b, c] = self.m;
        normalize(apply([cross(b, c), cross(c, a), cross(a, b)], n))
    }
}

/// A jelly's blows: when, where, and how hard. The small cube's landing,
/// each roll's, the fit and the reveal's burst.
fn blows() -> Vec<(f32, V3, f32)> {
    let mut blows = vec![(LANDS, top_of(START), 1.0)];
    for (i, &s) in PATH[..ON_TOP].iter().enumerate() {
        blows.push((ROLLS + i as f32 * STEP + ROLL, top_of(s), 0.55));
    }
    blows.push((FITS, [2.5, 3.0, 2.5], 0.9));
    blows.push((REVEAL, MIDDLE, 0.5));
    blows
}

/// A jelly's wobble: a column shakes this many times a second, the whole
/// cube's lean `SWAYS`, each losing most of it every `DAMP` (and
/// `SWAY_DAMP`) seconds; a blow travels `WAVE` blocks a second and fades
/// over `REACH` blocks; and from `QUIET` on, it's still.
const WOBBLES: f32 = 3.2;
const DAMP: f32 = 0.38;
const SWAYS: f32 = 2.2;
const SWAY_DAMP: f32 = 0.5;
const WAVE: f32 = 7.0;
const REACH: f32 = 1.3;
const QUIET: f32 = 8.4;
/// How hard a blow squashes the column it strikes (its height times as
/// much less), and the small cube; how far it sways the big cube's top
/// away from it; and how far a column's top face sinks as it's squashed
/// (and rises as it's stretched), for each part of its height it loses.
const COLUMN: f32 = 0.2;
const SMALL: f32 = 0.22;
const SWAY: f32 = 0.16;
const DIP: f32 = 0.3;
/// A squashed column widens as its height to this power less (keeping
/// its size would take a half): so that its blocks never reach their
/// neighbours.
const WIDEN: f32 = 0.35;

/// How far a jelly `away` blocks from a blow `since` seconds ago is
/// squashed (more than 0) or stretched by it, shaking `wobbles` times a
/// second and losing most of it each `damp` seconds.
fn wobble(since: f32, away: f32, wobbles: f32, damp: f32) -> f32 {
    let since = since - away / WAVE;
    if since < 0.0 {
        return 0.0;
    }
    (TAU * wobbles * since).sin() * (-since / damp).exp() * (-away / REACH).exp()
}

/// How jelly gives under the small cube's blows `t` seconds in: each column
/// squashed from the bottom (its height times [`Give::squash`]), the whole
/// leaning (its top moved `lean` sideways), and the small cube squashed on
/// its own. Nothing gives for other stuff, nor from `QUIET` on.
#[derive(Debug, Clone, Copy)]
struct Give {
    on: bool,
    t: f32,
    fade: f32,
    lean: [f32; 2],
}

impl Give {
    fn new(stuff: Stuff, t: f32) -> Self {
        let on = stuff == Stuff::Jelly && t < QUIET;
        let fade = 1.0 - smooth(QUIET - 1.6, QUIET, t);
        let mut lean = [0.0; 2];
        if on {
            for (when, at, hard) in blows() {
                // Away from where it struck, as far as that's off the
                // middle.
                let off = [MIDDLE[0] - at[0], MIDDLE[2] - at[2]];
                let k = hard * SWAY * fade * wobble(t - when, 0.0, SWAYS, SWAY_DAMP) / (off[0] * off[0] + off[1] * off[1]).sqrt().max(1.0);
                lean = [lean[0] + off[0] * k, lean[1] + off[1] * k];
            }
        }
        Self { on, t, fade, lean }
    }

    /// The height of the column at `x`, `z` now, times its height at rest.
    fn squash(&self, x: f32, z: f32) -> f32 {
        if !self.on {
            return 1.0;
        }
        let s: f32 = blows().into_iter().filter(|b| self.t > b.0).map(|(when, at, hard)| hard * wobble(self.t - when, (x - at[0]).hypot(z - at[2]), WOBBLES, DAMP)).sum();
        1.0 - COLUMN * s * self.fade
    }

    /// The small cube's own height now, times its height at rest: squashed
    /// as it's struck, and stretched as it falls.
    fn small(&self) -> f32 {
        if !self.on {
            return 1.0;
        }
        let s: f32 = blows().into_iter().filter(|b| self.t > b.0).map(|(when, _, hard)| hard * wobble(self.t - when, 0.0, WOBBLES, DAMP)).sum();
        let mut squash = 1.0 - SMALL * s * self.fade;
        if self.t > FALL && self.t < LANDS {
            squash += 0.16 * ((self.t - FALL) / (LANDS - FALL)).powi(2);
        }
        squash
    }

    /// A block at `middle` placed as the jelly has given: its column
    /// squashed from the bottom and widened as much less, and leaning with
    /// the whole.
    fn place(&self, placed: Placed, middle: V3) -> Placed {
        if !self.on {
            return placed;
        }
        let s = self.squash(middle[0], middle[2]);
        let w = s.powf(-WIDEN);
        let [x, z] = self.lean.map(|l| l / 3.0);
        placed.then([[w, x, 0.0], [0.0, s, 0.0], [0.0, z, w]], [middle[0] * (1.0 - w), 0.0, middle[2] * (1.0 - w)])
    }

    /// How far a column's top face at `x`, `z` rises in its middle.
    fn rise(&self, x: f32, z: f32) -> f32 {
        DIP * (self.squash(x, z) - 1.0)
    }
}

/// How far Flow's light falls full on a face turned `n` (in the world)
/// way, 0 to 1, in quarters: a lit face's own light gives way as far, so
/// that the two don't add up to white. The eye is always above, so the top
/// is the most in the light.
fn in_light(cam: &Camera, n: V3) -> f32 {
    (((shade(cam.turn(n)) - 0.35) / 0.7).clamp(0.0, 1.0) * 4.0).round() / 4.0
}

/// How far an ember face's cracks open, half across, where they meet,
/// along them and where they reach its edge, as fractions of its flat
/// part's half side (each crack a little wider or narrower than these);
/// how far in from them its plates' edges are warm; and how far its glow
/// reaches in under the rounds round it.
const MEET: f32 = 0.1;
const ALONG_CRACK: f32 = 0.045;
const AT_EDGE: f32 = 0.06;
const WARM: f32 = 0.09;
const UNDER: f32 = 0.03;
/// How far a plate's surface leans across it, for each half side.
const DOME: f32 = 0.35;

/// What a piece of an ember face is ([`ember_face`]), from the deepest:
/// the glow under the crust, the orange along each crack and its hot
/// core; plate `i`'s warm edge and its ashy middle; and a loose crack's
/// glow and core, over the crust.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ember {
    Deep,
    Orange,
    Core,
    Warm(usize),
    Ash(usize),
    LooseGlow,
    LooseCore,
}

impl Ember {
    /// How far above the face it lies, below it under 0: each a little
    /// above what it covers. The ash is the face itself, and the glow
    /// under it reaches in under the rounds, deeper than they come.
    fn lift(self) -> f32 {
        match self {
            Ember::Deep => -0.016,
            Ember::Orange => -0.012,
            Ember::Core => -0.008,
            Ember::Warm(_) => -0.004,
            Ember::Ash(_) => 0.0,
            Ember::LooseGlow => 0.004,
            Ember::LooseCore => 0.008,
        }
    }
}

/// A plate of an ember face's crust: the point it's domed about, how it
/// leans there, and its shade.
#[derive(Debug, Clone, Copy)]
struct Plate {
    top: [f32; 2],
    lean: [f32; 2],
    tone: i8,
}

/// A lit stone face's embers, in its plane, its flat part's half side 1:
/// convex pieces, counter-clockwise, and its plates.
struct EmberFace {
    pieces: Vec<(Ember, Vec<[f32; 2]>)>,
    plates: Vec<Plate>,
}

/// A crack between plates `i` and `j`: its points from one end to the
/// other as plate `i` goes round, each with how far it opens there, half
/// across (none where it's beyond the face); and whether any of it is on
/// the face.
struct Crack {
    i: usize,
    j: usize,
    points: Vec<([f32; 2], f32)>,
    seen: bool,
}

/// The embers of a lit stone face whose flat part's edge is `edge`
/// ([`face_edge`]), in a pattern of their own for each `seed`: a crust of
/// ash parted into plates round three to five points that fall where
/// they will, some off the face so their plates come in from its edge
/// (and no ring of crack round it); the cracks between them kinked,
/// opening widest where they meet and narrowest along their middles, and
/// glowing across them from a hot core, hottest toward the face's middle,
/// through orange to deep red, which warms the plates' edges beside it; a
/// loose crack or two running into a plate and dying away to a hair; and
/// under it all the deep red, reaching in under the rounds round the face,
/// so that nothing between them is open.
fn ember_face(seed: usize, edge: &[[f32; 2]]) -> EmberFace {
    let g = |k: usize| grain(seed, 200 + k);
    let face = fan_pieces([0.0; 2], edge);
    let under: Vec<[f32; 2]> = edge.iter().map(|&[a, b]| [a, b].map(|c| c * (1.0 + UNDER / a.hypot(b).max(1e-3)))).collect();
    let below = fan_pieces([0.0; 2], &under);
    let mut pieces: Vec<(Ember, Vec<[f32; 2]>)> = below.iter().map(|p| (Ember::Deep, p.clone())).collect();
    // A convex piece cut to `to`'s pieces, unless it's all within one of
    // them, which most are.
    let mut cut = |kind: Ember, poly: &[[f32; 2]], to: &[Vec<[f32; 2]>]| {
        if to.iter().any(|c| poly.iter().all(|p| (0..c.len()).all(|k| twice_area(&[c[k], c[(k + 1) % c.len()], *p]) >= 0.0))) {
            pieces.push((kind, poly.to_vec()));
            return;
        }
        for convex in to {
            let piece = within(poly, convex);
            if piece.len() >= 3 && twice_area(&piece) > 1e-6 {
                pieces.push((kind, piece));
            }
        }
    };

    // The plates' points: one near the middle, but not on it, and the rest
    // anywhere on the face or just off it, apart.
    let count = 3 + ((g(0) * 3.0) as usize).min(2);
    let mut seeds = vec![[0.8 * (g(1) - 0.5), 0.8 * (g(2) - 0.5)]];
    for k in 0..60 {
        let p = [2.6 * (g(3 + 2 * k) - 0.5), 2.6 * (g(4 + 2 * k) - 0.5)];
        if seeds.len() < count && seeds.iter().all(|s| (s[0] - p[0]).hypot(s[1] - p[1]) > 0.62) {
            seeds.push(p);
        }
    }
    let bound = [[-2.0, -2.0], [2.0, -2.0], [2.0, 2.0], [-2.0, 2.0]];
    let cells: Vec<Vec<[f32; 2]>> = seeds.iter().enumerate().map(|(i, &s)| seeds.iter().enumerate().filter(|&(j, _)| j != i).fold(bound.to_vec(), |c, (_, &o)| nearer(&c, s, o, 0.0))).collect();

    // The cracks: where two plates' cells meet, kinked once or twice where
    // the face shows them, as much as keeps each plate whole round its point.
    let mut cracks: Vec<Crack> = Vec::new();
    for (i, cell) in cells.iter().enumerate() {
        for k in 0..cell.len() {
            let (a, b) = (cell[k], cell[(k + 1) % cell.len()]);
            let Some(j) = (i + 1..seeds.len()).find(|&j| [a, b].iter().all(|p| (dist(*p, seeds[i]) - dist(*p, seeds[j])).abs() < 1e-4)) else { continue };
            let n = cracks.len();
            let h = |k: usize| grain(seed, 400 + 16 * n + k);
            let wide = 0.75 + 0.5 * h(0);
            let open = |p: [f32; 2]| if p[0].abs() < 1.0 && p[1].abs() < 1.0 { MEET * (0.85 + 0.3 * h(1)) } else { AT_EDGE * wide };
            let mut points = vec![(a, open(a))];
            let seen = match on_square(a, b) {
                Some((t0, t1)) => {
                    let at = |t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
                    if t0 > 0.0 {
                        points.push((at(t0), AT_EDGE * wide));
                    }
                    let length = dist(at(t0), at(t1));
                    let across = [-(b[1] - a[1]) / dist(a, b), (b[0] - a[0]) / dist(a, b)];
                    let most = 0.3 * dist(seeds[i], seeds[j]) * 0.5;
                    let kinks: &[f32] = if length > 1.3 { &[0.36, 0.68] } else { &[0.5] };
                    for (m, &t) in kinks.iter().enumerate() {
                        let t = t0 + (t1 - t0) * (t + 0.12 * (h(2 + m) - 0.5));
                        let side = if (h(5) < 0.5) == (m == 0) { 1.0 } else { -1.0 };
                        let off = side * (length * (0.07 + 0.08 * h(6 + m))).min(most);
                        let p = at(t);
                        points.push(([p[0] + across[0] * off, p[1] + across[1] * off], ALONG_CRACK * wide * (0.8 + 0.4 * h(8 + m))));
                    }
                    if t1 < 1.0 {
                        points.push((at(t1), AT_EDGE * wide));
                    }
                    true
                }
                None => false,
            };
            points.push((b, open(b)));
            cracks.push(Crack { i, j, points, seen });
        }
    }

    // The plates, each its cell with its cracks' kinks, drawn back from
    // them as far as each opens: a warm edge, and within it, as far again,
    // the ash, each leaning its own way.
    let mut plates = Vec::new();
    for (i, cell) in cells.iter().enumerate() {
        let mut ring: Vec<([f32; 2], f32)> = Vec::new();
        for k in 0..cell.len() {
            let (a, b) = (cell[k], cell[(k + 1) % cell.len()]);
            let other = (0..seeds.len()).find(|&o| o != i && [a, b].iter().all(|p| (dist(*p, seeds[i]) - dist(*p, seeds[o])).abs() < 1e-4));
            match other.and_then(|o| cracks.iter().find(|c| (c.i, c.j) == (i.min(o), i.max(o)))) {
                // (Not where it crosses the face's edge, which only
                // the glow needs.)
                Some(c) if c.i == i => ring.extend(c.points[..c.points.len() - 1].iter().filter(|p| !on_edge(p.0))),
                Some(c) => ring.extend(c.points.iter().rev().take(c.points.len() - 1).filter(|p| !on_edge(p.0))),
                None => ring.push((a, 0.0)),
            }
        }
        let s = seeds[i];
        let back = |extra: f32| -> Vec<[f32; 2]> {
            ring.iter()
                .map(|&(p, open)| {
                    let k = if open > 0.0 { ((open + extra) / dist(p, s).max(1e-3)).min(0.9) } else { 0.0 };
                    [p[0] + (s[0] - p[0]) * k, p[1] + (s[1] - p[1]) * k]
                })
                .collect()
        };
        for (kind, extra) in [(Ember::Warm(i), 0.0), (Ember::Ash(i), WARM)] {
            for piece in fan_pieces(s, &back(extra)) {
                cut(kind, &piece, &face);
            }
        }
        let h = |k: usize| grain(seed, 700 + 8 * i + k);
        let tone = match h(0) {
            x if x < 0.3 => -1,
            x if x > 0.7 => 1,
            _ => 0,
        };
        plates.push(Plate { top: s.map(|c| c.clamp(-0.7, 0.7)), lean: [0.5 * (h(1) - 0.5), 0.5 * (h(2) - 0.5)], tone });
    }

    // The glow along each crack, under the plates' edges: orange, and its
    // core, hotter toward the face's middle; each piece of it a little
    // longer than its stretch, so that they meet at the kinks.
    let heat = |p: [f32; 2]| (1.2 - 0.5 * p[0].hypot(p[1])).clamp(0.45, 1.1);
    for crack in cracks.iter().filter(|c| c.seen) {
        // (Not off the face, which its pieces either side reach past.)
        for w in crack.points.windows(2).filter(|w| w.iter().all(|p| p.0[0].abs().max(p.0[1].abs()) < 1.0 + 1e-5)) {
            let ((a, ga), (b, gb)) = (w[0], w[1]);
            cut(Ember::Orange, &band(a, b, 0.55 * ga, 0.55 * gb), &face);
            if heat(a).min(heat(b)) > 0.6 {
                cut(Ember::Core, &band(a, b, 0.24 * ga * heat(a), 0.24 * gb * heat(b)), &face);
            }
        }
    }

    // A loose crack or two, from a kink of one into a plate beside it,
    // dying away to a hair.
    let kinks: Vec<(usize, [f32; 2], f32)> = cracks.iter().filter(|c| c.seen).flat_map(|c| c.points.iter().filter(|p| p.0[0].abs() < 0.8 && p.0[1].abs() < 0.8 && p.1 < MEET * 0.8).map(move |p| (if grain(seed, 900 + c.i * 7 + c.j) < 0.5 { c.i } else { c.j }, p.0, p.1))).collect();
    let loose = if kinks.is_empty() { 0 } else { 1 + usize::from(g(150) > 0.45) };
    for k in 0..loose {
        let h = |m: usize| grain(seed, 1000 + 8 * k + m);
        let (plate, from, open) = kinks[((h(0) * kinks.len() as f32) as usize + k * kinks.len() / 2) % kinks.len()];
        let s = seeds[plate];
        let turn = (0.35 + 0.4 * h(1)) * if h(2) < 0.5 { 1.0 } else { -1.0 };
        let (sin, cos) = turn.sin_cos();
        let to = [s[0] - from[0], s[1] - from[1]];
        let length = dist(s, from) * (0.4 + 0.25 * h(3));
        let way = [to[0] * cos - to[1] * sin, to[0] * sin + to[1] * cos].map(|c| c / dist(s, from).max(1e-3));
        let end = [from[0] + way[0] * length, from[1] + way[1] * length];
        let bend = 0.18 * length * (h(4) - 0.5);
        let mid = [(from[0] + end[0]) * 0.5 - way[1] * bend, (from[1] + end[1]) * 0.5 + way[0] * bend];
        let (wide, middle, hair) = (0.5 * open, 0.3 * open, 0.004);
        for (a, b, wa, wb) in [(from, mid, wide, middle), (mid, end, middle, hair)] {
            cut(Ember::LooseGlow, &band(a, b, wa, wb), &face);
            cut(Ember::LooseCore, &band(a, b, 0.4 * wa, 0.4 * wb), &face);
        }
    }
    EmberFace { pieces, plates }
}

/// Whether a point is on the edge of the square of half side 1.
fn on_edge(p: [f32; 2]) -> bool {
    (p[0].abs().max(p[1].abs()) - 1.0).abs() < 1e-5
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Twice a polygon's area, more than 0 where it goes counter-clockwise.
fn twice_area(poly: &[[f32; 2]]) -> f32 {
    (0..poly.len()).map(|i| (poly[i], poly[(i + 1) % poly.len()])).map(|(a, b)| a[0] * b[1] - b[0] * a[1]).sum()
}

/// Where the line from `a` to `b` is on the square of half side 1, as how
/// far along it it comes on and goes off.
fn on_square(a: [f32; 2], b: [f32; 2]) -> Option<(f32, f32)> {
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for k in 0..2 {
        let d = b[k] - a[k];
        for (p, q) in [(-d, a[k] + 1.0), (d, 1.0 - a[k])] {
            if p.abs() < 1e-9 {
                if q < 0.0 {
                    return None;
                }
            } else if p < 0.0 {
                t0 = t0.max(q / p);
            } else {
                t1 = t1.min(q / p);
            }
        }
    }
    (t1 - t0 > 1e-4).then_some((t0, t1))
}

/// A band from `a` to `b`, `wa` either side of it at `a` and `wb` at `b`,
/// as far again past each end: counter-clockwise.
fn band(a: [f32; 2], b: [f32; 2], wa: f32, wb: f32) -> Vec<[f32; 2]> {
    let l = dist(a, b).max(1e-6);
    let (d, n) = ([(b[0] - a[0]) / l, (b[1] - a[1]) / l], [-(b[1] - a[1]) / l, (b[0] - a[0]) / l]);
    let at = |p: [f32; 2], along: f32, across: f32| [p[0] + d[0] * along + n[0] * across, p[1] + d[1] * along + n[1] * across];
    vec![at(a, -wa, -wa), at(b, wb, -wb), at(b, wb, wb), at(a, -wa, wa)]
}

/// The polygon round `middle` whose corners are `ring`, counter-clockwise
/// round it, as convex pieces: as many of the triangles from `middle` to
/// its sides as stay convex together, each.
fn fan_pieces(middle: [f32; 2], ring: &[[f32; 2]]) -> Vec<Vec<[f32; 2]>> {
    let n = ring.len();
    let convex = |poly: &[[f32; 2]]| {
        let m = poly.len();
        (0..m).all(|k| {
            let (a, b, c) = (poly[k], poly[(k + 1) % m], poly[(k + 2) % m]);
            (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]) >= -1e-6
        })
    };
    if convex(ring) {
        return vec![ring.to_vec()];
    }
    // From a corner where it turns in, so that none of the pieces does.
    let turns_in = |k: usize| {
        let (a, b, c) = (ring[(k + n - 1) % n], ring[k], ring[(k + 1) % n]);
        (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]) < 0.0
    };
    let start = (0..n).find(|&k| turns_in(k)).unwrap_or(0);
    let mut pieces = Vec::new();
    let mut piece = vec![middle, ring[start]];
    for k in 1..=n {
        let next = ring[(start + k) % n];
        piece.push(next);
        if piece.len() > 3 && !convex(&piece) {
            piece.pop();
            let last = *piece.last().expect("a corner");
            pieces.push(std::mem::replace(&mut piece, vec![middle, last, next]));
        }
    }
    pieces.push(piece);
    pieces.retain(|p| twice_area(p) > 1e-7);
    pieces
}

/// What draws a frame's blocks: the camera, what they're made of, and what
/// went in.
struct Brush<'a> {
    scene: &'a mut Scene,
    cam: &'a Camera,
    stuff: Stuff,
    drawn: Drawn,
}

/// A frame's see-through triangles, and how many of them are tinted (the
/// F's faces: the rest are one glass); and the solid core's triangles.
#[derive(Debug, Clone, Copy, Default)]
struct Drawn {
    see_through: usize,
    tinted: usize,
    core: usize,
}

/// A shell's corners where a block's placed: where they show, and the
/// way they face in the world.
struct Seen {
    shown: Vec<V3>,
    normal: Vec<V3>,
}

impl Seen {
    /// Whether the eye sees the side of a triangle its corners go
    /// counter-clockwise round.
    fn front(&self, v: [u32; 3]) -> bool {
        let [a, b, c] = v.map(|i| self.shown[i as usize]);
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) > 0.0
    }
}

impl Brush<'_> {
    /// Where `points` show, if all are in front of the eye, and whether the
    /// eye sees the side they go counter-clockwise round.
    fn shown(&self, points: &[V3]) -> Option<(Vec<V3>, bool)> {
        let shown: Vec<V3> = points.iter().map(|&p| self.cam.project(p)).collect::<Option<_>>()?;
        let n = shown.len();
        let twice_area: f32 = (0..n).map(|i| (shown[i], shown[(i + 1) % n])).map(|(a, b)| a[0] * b[1] - b[0] * a[1]).sum();
        Some((shown, twice_area > 0.0))
    }

    /// A polygon where `shown`, facing `normals` (in the world) at its
    /// corners.
    fn put(&mut self, material: Material, shown: &[V3], normals: &[V3]) {
        let corners: Vec<Corner> = shown.iter().zip(normals).map(|(&at, &n)| Corner { at, normal: self.cam.turn(n) }).collect();
        let before = self.scene.triangles();
        self.scene.polygon(material, &corners);
        let added = self.scene.triangles() - before;
        if material.color[3] < 1.0 {
            self.drawn.see_through += added;
            if Some(material) != self.stuff.inner() {
                self.drawn.tinted += added;
            }
        }
    }

    /// How many pixels `length` at `placed`'s middle takes on the screen,
    /// at the most, whichever way it goes.
    fn pixels(&self, placed: &Placed, length: f32) -> f32 {
        let Some(middle) = self.cam.project(placed.point([0.0; 3])) else { return 0.0 };
        WAYS[..6].iter().step_by(2).filter_map(|&w| self.cam.project(placed.point(mul(w, length)))).map(|p| (p[0] - middle[0]).hypot(p[1] - middle[1])).fold(0.0, f32::max)
    }

    /// About how much of a block where `placed` puts it is in the
    /// picture, 0 to 1: how much of a square round it is.
    fn in_view(&self, placed: &Placed) -> f32 {
        let reach = self.pixels(placed, 0.6);
        let Some(p) = self.cam.project(placed.point([0.0; 3])) else { return 0.0 };
        let within = |at: f32, half: f32| ((at + reach).min(half) - (at - reach).max(-half)).max(0.0) / (2.0 * reach).max(1e-3);
        within(p[0], self.scene.width * 0.5) * within(p[1], self.scene.height * 0.5)
    }

    /// `shell`'s corners where `placed` puts them.
    fn seen(&self, shell: &Shell, placed: &Placed) -> Option<Seen> {
        let shown = shell.at.iter().map(|&p| self.cam.project(placed.point(p))).collect::<Option<Vec<_>>>()?;
        Some(Seen { shown, normal: shell.normal.iter().map(|&n| placed.normal(n)).collect() })
    }

    /// `tris`, each in its material, as fans. A fan reaching past the
    /// picture's edge goes as its triangles, which the scene cuts to it.
    fn fans(&mut self, seen: &Seen, tris: &[([u32; 3], Material)]) {
        let mut materials: Vec<Material> = Vec::new();
        for (_, m) in tris {
            if !materials.contains(m) {
                materials.push(*m);
            }
        }
        let (w, h) = (self.scene.width * 0.5, self.scene.height * 0.5);
        for m in materials {
            let these: Vec<[u32; 3]> = tris.iter().filter(|t| t.1 == m).map(|t| t.0).collect();
            for fan in fans(&these, seen.shown.len()) {
                let shown: Vec<V3> = fan.iter().map(|&i| seen.shown[i as usize]).collect();
                let normals: Vec<V3> = fan.iter().map(|&i| seen.normal[i as usize]).collect();
                if shown.iter().all(|p| p[0].abs() <= w && p[1].abs() <= h) {
                    self.put(m, &shown, &normals);
                } else {
                    for k in 1..fan.len() - 1 {
                        self.put(m, &[shown[0], shown[k], shown[k + 1]], &[normals[0], normals[k], normals[k + 1]]);
                    }
                }
            }
        }
    }

    /// A block as `plan` has it, all of it its own material but the flat
    /// faces `lit` says are lit (and how far into their flash) and those
    /// that are walls of the notch. What faces the eye is drawn; where it's
    /// see-through, so is the inside of its far side, where it's beside a
    /// face turned away.
    fn block(&mut self, plan: &Plan, lit: impl Fn(usize) -> Option<f32>) {
        let (n, placed, form, open, own) = (plan.n, &plan.placed, &plan.form, plan.open, plan.own);
        let stuff = self.stuff;
        let near = shell(form, open);
        let Some(seen) = self.seen(&near, placed) else { return };
        let lights: [Option<Glowing>; 6] = std::array::from_fn(|f| lit(f).filter(|_| open[f]).map(|flash| stuff.lit(flash, in_light(self.cam, placed.normal(WAYS[f])))));
        let mut facing = 0u8;
        let mut front = Vec::new();
        for t in near.tris.iter().filter(|t| seen.front(t.v)) {
            let material = match t.face.and_then(|f| lights[f].map(|g| (f, g))) {
                Some((f, glowing)) => {
                    facing |= 1 << f;
                    match stuff.lit_cover() {
                        Some(cover) => cover,
                        // Stone's lit faces are embers, drawn below.
                        None if stuff == Stuff::Rock => continue,
                        None => glowing.heart,
                    }
                }
                // The notch's walls and floor, and the rounds round them,
                // in shadow, for the see-through stuff.
                None => match (0..6).find(|&f| plan.walls >> f & 1 == 1 && t.beside >> f & 1 == 1).and_then(|f| stuff.notch(plan.world[f] == TOP)) {
                    Some(shadow) => shadow,
                    None if t.tone != 0 => fleck(own, t.tone),
                    None => own,
                },
            };
            front.push((t.v, material));
        }
        self.fans(&seen, &front);
        for (f, glowing) in lights.iter().enumerate() {
            match glowing {
                Some(glowing) if facing >> f & 1 == 1 && stuff == Stuff::Rock => self.embers(n, placed, form, f, glowing),
                Some(glowing) if facing >> f & 1 == 1 && stuff.see_through() => self.light(placed, form, f, glowing),
                _ => {}
            }
        }
        // The far side: the faces turned away, and their rounded edges and
        // corners, seen from inside. (Those that only line the gaps between
        // blocks, beside faces turned to the eye, add triangles and little
        // else.) It's seen only through the near side, so it's cut more
        // coarsely where that's finely cut ([`far_steps`]). It's lit as
        // facing away, as it does, so that the layers behind the nearest
        // are darker and the nearest stands out.
        let Some(inner) = stuff.inner() else { return };
        let far_form = Form { steps: far_steps(form.steps), near: Some(form.steps), ..*form };
        let far = if far_form.steps == form.steps { None } else { Some(shell(&far_form, open)) };
        let (far, seen) = match &far {
            Some(far) => match self.seen(far, placed) {
                Some(seen) => (far, seen),
                None => return,
            },
            None => (&near, seen),
        };
        let away = far.tris.iter().filter(|t| !seen.front(t.v)).filter_map(|t| t.face).fold(0u8, |a, f| a | 1 << f);
        let back: Vec<([u32; 3], Material)> = far.tris.iter().filter(|t| t.beside & away != 0 && !seen.front(t.v)).map(|t| ([t.v[0], t.v[2], t.v[1]], inner)).collect();
        self.fans(&seen, &back);
    }

    /// A see-through block's lit face `f`: its light, a bright heart in a
    /// larger, dimmer halo sunk deeper ([`Stuff::light_layers`]), both
    /// rounded squares, so that the light falls off toward the face's
    /// edges, below the lowest its face sinks as jelly wobbles.
    fn light(&mut self, placed: &Placed, form: &Form, f: usize, glowing: &Glowing) {
        let out = WAYS[f];
        let (u, v) = (along(out), cross(out, along(out)));
        let normal = placed.normal(out);
        let face = form.half + if dot(out, form.up) > 0.9 { form.rise.min(0.0) } else { 0.0 };
        let [(halo, halo_sink), (heart, heart_sink)] = self.stuff.light_layers(form);
        for (material, reach, sink) in [(glowing.halo, halo, halo_sink), (Some(glowing.heart), heart, heart_sink)] {
            let Some(material) = material else { continue };
            let points: Vec<V3> = rounded_square(reach, 0.45, ROUNDED).iter().map(|&[a, b]| placed.point(add(mul(out, face - sink), add(mul(u, a), mul(v, b))))).collect();
            if let Some((shown, true)) = self.shown(&points) {
                self.put(material, &shown, &vec![normal; shown.len()]);
            }
        }
    }

    /// Stone block `n`'s lit face `f`: embers ([`ember_face`]), out to its
    /// flat part's edge as it's chipped, the glow under the crust reaching
    /// in under the rounds round it, so that nothing between them is open.
    /// Each plate of crust leans its own way and is domed a little, so the
    /// light falls across it.
    fn embers(&mut self, n: usize, placed: &Placed, form: &Form, f: usize, glowing: &Glowing) {
        let out = WAYS[f];
        let (u, v) = (along(out), cross(out, along(out)));
        let inner = form.half - form.round;
        let stone = form.facets.unwrap_or(n);
        let at = |[a, b]: [f32; 2], lift: f32| placed.point(add(mul(out, form.half + tilt(stone, f, a, b) + lift), add(mul(u, a * inner), mul(v, b * inner))));
        let face = ember_face(stone * 6 + f, &face_edge(form, stone, f));
        let normal = placed.normal(out);
        let deep = glowing.halo.unwrap_or(glowing.heart);
        let orange = blend(glowing.heart, deep, 0.5);
        let crust = glowing.crust.unwrap_or(deep);
        let ash = glowing.ash.unwrap_or(crust);
        for (kind, poly) in &face.pieces {
            let material = match *kind {
                Ember::Deep | Ember::LooseGlow => deep,
                Ember::Orange | Ember::LooseCore => orange,
                Ember::Core => glowing.heart,
                Ember::Warm(_) => crust,
                Ember::Ash(i) => fleck(ash, face.plates[i].tone),
            };
            let normals: Vec<V3> = match *kind {
                Ember::Warm(i) | Ember::Ash(i) => {
                    let plate = &face.plates[i];
                    poly.iter().map(|&[a, b]| placed.normal(normalize(add(out, add(mul(u, plate.lean[0] + DOME * (a - plate.top[0])), mul(v, plate.lean[1] + DOME * (b - plate.top[1]))))))).collect()
                }
                _ => vec![normal; poly.len()],
            };
            let points: Vec<V3> = poly.iter().map(|&p| at(p, kind.lift())).collect();
            if let Some((shown, true)) = self.shown(&points) {
                self.put(material, &shown, &normals);
            }
        }
    }

    /// The dark core that closes the gaps between solid blocks, as a real
    /// cube's does: a square in each of the six planes between layers,
    /// from `lo` to `hi` (just under the blocks' outer faces), the notch's
    /// square left out of the three round it until it's `whole`, so the
    /// blocks' faces that look into it show.
    fn core(&mut self, material: Material, (lo, hi): (f32, f32), whole: bool) {
        for k in 0..3 {
            let (i, j) = ((k + 1) % 3, (k + 2) % 3);
            for c in [1.0, 2.0] {
                let squares: &[[f32; 4]] = if c == 2.0 && !whole { &[[lo, hi, lo, 2.0], [lo, 2.0, 2.0, hi]] } else { &[[lo, hi, lo, hi]] };
                for &[a0, a1, b0, b1] in squares {
                    let at = |a: f32, b: f32| {
                        let mut p = [0.0; 3];
                        (p[k], p[i], p[j]) = (c, a, b);
                        p
                    };
                    let Some((shown, front)) = self.shown(&[at(a0, b0), at(a1, b0), at(a1, b1), at(a0, b1)]) else { continue };
                    let mut n = [0.0; 3];
                    n[k] = if front { 1.0 } else { -1.0 };
                    let before = self.scene.triangles();
                    self.put(material, &shown, &[n; 4]);
                    self.drawn.core += self.scene.triangles() - before;
                }
            }
        }
    }
}

/// A block of a frame, before it's drawn: its number, where it is in the
/// big cube (none for the small cube rolling on it), which way each of its
/// own faces looks in the world (by [`WAYS`]), how big its rounds are on
/// the screen and how much of it is in the picture, where it's placed, its
/// shape, what of it is open, what it's made of, and which of its faces
/// are walls of the notch, a bit each.
struct Plan {
    n: usize,
    block: Option<[i32; 3]>,
    world: [usize; 6],
    pixels: f32,
    in_view: f32,
    placed: Placed,
    form: Form,
    open: [bool; 6],
    own: Material,
    walls: u8,
}

/// About how many see-through triangles a frame may have, as [`cost`]
/// counts them: Flow cuts them where they cross and sorts them as it loads
/// each frame, which on a Mac takes about a millisecond for so many; and
/// it cuts no more than 2,048 a frame for a model that moves by itself
/// (its `EACH_FRAME`), which a frame of the cube keeps under too, with the
/// stage's two corners. Jelly's frames have a few more than [`cost`]
/// counts, glass's a few fewer, so jelly's is less: a play has about
/// 2,040 at most in either, at the shapes of picture tried.
const SEE_THROUGH: usize = 2050;
const SEE_THROUGH_JELLY: usize = 1960;

/// How finely each block's rounds are cut: as finely as their size on the
/// screen asks, up to the most its stuff allows; but see-through stuff no
/// more finely than keeps a frame's see-through triangles to about
/// [`SEE_THROUGH`] (or [`SEE_THROUGH_JELLY`]): the most finely cut made
/// coarser first, the smallest on the screen of those first.
fn cut(blocks: &mut [Plan], stuff: Stuff) {
    for b in blocks.iter_mut() {
        b.form.steps = steps_for(b.pixels, stuff.most_steps());
    }
    if !stuff.see_through() {
        return;
    }
    // Not five, whose far side would be cut as finely as its near: six,
    // or four. And not three: a near side of three has a far side of
    // three, which takes as many triangles as four's near side and its far
    // side of two ([`cost`]), so four, rounder for the same.
    for b in blocks.iter_mut() {
        b.form.steps = match b.form.steps {
            5 if stuff.most_steps() >= 6 => 6,
            3 | 5 if stuff.most_steps() >= 4 => 4,
            s => s,
        };
    }
    let budget = if stuff == Stuff::Jelly { SEE_THROUGH_JELLY } else { SEE_THROUGH } as f32;
    while blocks.iter().map(|b| cost(b.open, b.form.steps) as f32 * b.in_view).sum::<f32>() > budget {
        let Some(b) = blocks.iter_mut().filter(|b| b.in_view > 0.0 && b.form.steps > 2).max_by(|a, b| a.form.steps.cmp(&b.form.steps).then(b.pixels.total_cmp(&a.pixels))) else { break };
        b.form.steps = match b.form.steps {
            6 => 4,
            4 => 2,
            s => s - 1,
        };
    }
}

/// About how many see-through triangles a block `open` where it is, cut in
/// `steps`, takes: half its near side's, which face the eye, and half its
/// far side's, cut more coarsely where it's finely cut.
fn cost(open: [bool; 6], steps: usize) -> usize {
    (surface(open, steps, steps) + surface(open, far_steps(steps), steps)) / 2
}

/// How many triangles a block's surface `open` where it is, cut in
/// `steps`, has: its near side's, or its far side's where its near side is
/// cut in `near`.
fn surface(open: [bool; 6], steps: usize, near: usize) -> usize {
    let faces = 2 * open.iter().filter(|&&o| o).count();
    let edges = (0..6).flat_map(|a| (a + 1..6).map(move |b| (a, b))).filter(|&(a, b)| a / 2 != b / 2 && (open[a] || open[b])).count();
    let corners: usize = (0..8usize)
        .map(|s| [open[s & 1], open[2 + (s >> 1 & 1)], open[4 + (s >> 2 & 1)]].iter().filter(|&&o| o).count())
        .map(|k| match k {
            0 => 0,
            _ if fanned(near) => 3 * steps,
            _ => steps * steps,
        })
        .sum();
    faces + 2 * steps * edges + corners
}

/// Block `n`'s place in the big cube: across, up and back to front.
fn cell(n: usize) -> [i32; 3] {
    [n as i32 % 3, n as i32 / 3 % 3, n as i32 / 9]
}

/// The number of the block whose middle is `middle`.
fn cell_of(middle: V3) -> usize {
    let [x, y, z] = middle.map(|c| (c - 0.5).round().clamp(0.0, 2.0) as usize);
    x + 3 * y + 9 * z
}

/// Which of the big cube's block `block`'s faces are open (by [`WAYS`]):
/// on the big cube's outside, or looking into the notch while it's empty;
/// and which of them look into it, a bit each, its floor and walls until
/// the small cube fills it (`whole`). None for a block that isn't drawn:
/// the middle one, and the notch while it's empty.
fn faces_of(block: [i32; 3], whole: bool) -> Option<([bool; 6], u8)> {
    if block == [1, 1, 1] || (block == NOTCH && !whole) {
        return None;
    }
    let mut walls = 0;
    let open = std::array::from_fn(|f| {
        let next = [0, 1, 2].map(|k| block[k] + WAYS[f][k] as i32);
        let into_notch = next == NOTCH && !whole;
        if into_notch {
            walls |= 1 << f;
        }
        into_notch || next.iter().any(|c| !(0..3).contains(c))
    });
    Some((open, walls))
}

pub struct Cube {
    t: f32,
    stuff: Stuff,
    /// Where the eye looks while it follows the small cube.
    follow: V3,
    /// The small cube's turn before each roll, worked out once.
    turns: Vec<M3>,
    /// How many squares along the F's strokes each tile is from the top's
    /// F, by face, row and column: none for a tile that isn't in an F.
    steps: [[[Option<u8>; 3]; 3]; 6],
    /// A key changed it since the last frame, which comes at once.
    keyed: bool,
}

impl Cube {
    pub fn new() -> Self {
        Self::of(Stuff::Glass)
    }

    /// The cube made of `stuff`, from the start.
    fn of(stuff: Stuff) -> Self {
        let mut turns = vec![IDENTITY];
        let mut at = START;
        for &next in &PATH[..ON_TOP] {
            let (axis, _) = roll_axis(at, next);
            let last = *turns.last().expect("one at least");
            turns.push(compose(rotation(axis, FRAC_PI_2), last));
            at = next;
        }
        // The light runs out from the top's F along the strokes of each
        // face's F, over the edges where they meet.
        let mut steps = [[[None; 3]; 3]; 6];
        let mut queue: VecDeque<Tile> = Tile::all().filter(|t| t.face == TOP && t.in_f()).collect();
        for t in &queue {
            steps[t.face][t.row as usize][t.col as usize] = Some(0);
        }
        while let Some(t) = queue.pop_front() {
            let n = steps[t.face][t.row as usize][t.col as usize].expect("reached");
            for next in Tile::all().filter(|u| u.in_f()) {
                let far = sub(next.middle(), t.middle());
                if steps[next.face][next.row as usize][next.col as usize].is_none() && dot(far, far) < 1.01 {
                    steps[next.face][next.row as usize][next.col as usize] = Some(n + 1);
                    queue.push_back(next);
                }
            }
        }
        Self { t: 0.0, stuff, follow: top_of(START), turns, steps, keyed: false }
    }

    /// The small cube now: its center and its turn.
    fn small(&self) -> (V3, M3) {
        let t = self.t;
        if t < ROLLS {
            let height = if t < LANDS {
                let k = ((t - FALL) / (LANDS - FALL)).clamp(0.0, 1.0);
                5.0 * (1.0 - k * k)
            } else {
                // A little hop after it lands.
                0.22 * (PI * ((t - LANDS) / 0.25).min(1.0)).sin()
            };
            return (add(above(START), [0.0, height, 0.0]), IDENTITY);
        }
        if t < TIPS {
            let i = (((t - ROLLS) / STEP) as usize).min(ON_TOP);
            let from = if i == 0 { START } else { PATH[i - 1] };
            if i == ON_TOP {
                return (above(from), self.turns[i]);
            }
            let k = smooth(0.0, 1.0, ((t - ROLLS - i as f32 * STEP) / ROLL).min(1.0));
            let (axis, pivot) = roll_axis(from, PATH[i]);
            let spin = rotation(axis, FRAC_PI_2 * k);
            return (add(pivot, apply(spin, sub(above(from), pivot))), compose(spin, self.turns[i]));
        }
        // Over the edge: up onto it as a roll goes, then a half turn on
        // the same edge down into the notch, faster as it falls.
        let k = ((t - TIPS) / TIP).min(1.0);
        if k < 1.0 {
            let from = PATH[ON_TOP - 1];
            let (axis, pivot) = roll_axis(from, PATH[ON_TOP]);
            let spin = rotation(axis, PI * k * k);
            return (add(pivot, apply(spin, sub(above(from), pivot))), compose(spin, self.turns[ON_TOP]));
        }
        // In, square to the world, with a little hop as it settles.
        let (axis, _) = roll_axis(PATH[ON_TOP - 1], PATH[ON_TOP]);
        let turn = compose(rotation(axis, PI), self.turns[ON_TOP]).map(|row| row.map(f32::round));
        let hop = 0.06 * (PI * ((t - FITS) / SETTLE).min(1.0)).sin();
        (add(IN_NOTCH, [0.0, hop, 0.0]), turn)
    }

    /// When a square of the top lit up, if it has.
    fn lit_at(&self, square: Square) -> Option<f32> {
        let mut when = (square == START).then_some(LANDS);
        for (i, &s) in PATH.iter().take(LIGHTING).enumerate() {
            if s == square && when.is_none() {
                when = Some(ROLLS + i as f32 * STEP + ROLL);
            }
        }
        when.filter(|&w| self.t >= w)
    }

    /// When a tile lights, if it has: the top's F as the small cube rolls
    /// it out, the rest as the light reaches them.
    fn lit(&self, tile: Tile) -> Option<f32> {
        if tile.face == TOP {
            return self.lit_at((tile.col, tile.row));
        }
        let steps = self.steps[tile.face][tile.row as usize][tile.col as usize]?;
        Some(REVEAL + SPREAD * f32::from(steps)).filter(|&w| self.t >= w)
    }

    /// Whether a tile's face is lit now, and if it is how far into its
    /// flash: it comes on bright and pale and settles. The top's F flashes
    /// again as the light sets off from it.
    fn look(&self, tile: Tile) -> Option<f32> {
        let when = self.lit(tile)?;
        let mut flash = 1.0 - smooth(when, when + FLASH, self.t);
        if tile.face == TOP && self.t >= REVEAL {
            flash = flash.max(1.0 - smooth(REVEAL, REVEAL + FLASH, self.t));
        }
        Some(flash)
    }

    fn camera(&self, width: f32, height: f32) -> Camera {
        camera_at(self.t, self.follow, width / height, height)
    }

    /// The landings, the fit and the reveal between `before` and now.
    fn effects(&self, before: f32, cam: &Camera, w: f32) -> Vec<Fx> {
        let crossed = |at: f32| before < at && at <= self.t;
        let shown = |p: V3| cam.project(p).map(|p| [p[0], p[1]]);
        let mut fx = Vec::new();
        if crossed(LANDS) {
            fx.extend(shown(top_of(START)).map(|at| Fx::new(Kind::Ripple, at, w * 0.08, 0.5).sound(Sound::Thud)));
        }
        for (i, &s) in PATH[..ON_TOP].iter().enumerate() {
            if crossed(ROLLS + i as f32 * STEP + ROLL) {
                fx.extend(shown(top_of(s)).map(|at| Fx::new(Kind::Ripple, at, w * 0.05, 0.3).sound(Sound::Thud)));
            }
        }
        if crossed(FITS) {
            fx.extend(shown(IN_NOTCH).map(|at| Fx::new(Kind::Ripple, at, w * 0.1, 0.6).sound(Sound::Thud)));
        }
        if crossed(REVEAL) {
            if let Some(at) = shown(MIDDLE) {
                fx.push(Fx::new(Kind::Burst, at, w * 0.3, 0.8).color(self.stuff.light()).sound(Sound::Chime));
                fx.push(Fx::new(Kind::Ripple, at, w * 0.45, 0.6).quiet());
            }
        }
        fx
    }

    /// The cube now, seen through `cam`: the big cube's blocks, the small
    /// one, and the sparkles; and the see-through triangles it took.
    fn draw(&self, scene: &mut Scene, cam: &Camera, h: f32) -> Drawn {
        let stuff = self.stuff;
        let (small, turn) = self.small();
        // From the fit on, the small cube is the big cube's missing block,
        // square to the world: the faces between them are inside.
        let whole = self.t >= FITS;
        let give = Give::new(stuff, self.t);
        let mut brush = Brush { scene, cam, stuff, drawn: Drawn::default() };
        if let Some(core) = stuff.core() {
            let reach = stuff.shape(0).0 - 0.02;
            brush.core(core, (0.5 - reach, 2.5 + reach), whole);
        }
        // Each block: where it is, its shape, what of it is open and what
        // it's made of.
        let mut blocks = Vec::with_capacity(27);
        for n in 0..27 {
            let block = cell(n);
            let Some((open, walls)) = faces_of(block, whole) else { continue };
            let mut middle = block.map(|c| c as f32 + 0.5);
            let mut own = stuff.body(n);
            let (mut at, mut world) = (Placed::still(middle), [0, 1, 2, 3, 4, 5]);
            if block == NOTCH {
                // Settling into place, it becomes one of the big cube's
                // blocks, turned as it rolled in, so that stone's chips
                // stay where they were.
                middle = small;
                at = Placed { m: turn, at: small };
                world = std::array::from_fn(|f| WAYS.iter().position(|&w| dot(w, apply(turn, WAYS[f])) > 0.9).expect("square to the world"));
                let k = smooth(FITS, FITS + SETTLE + 0.1, self.t);
                own = blend(stuff.small(), own, (k * 8.0).round() / 8.0);
            }
            let (open, walls) = (world.map(|w| open[w]), (0..6).filter(|&f| walls >> world[f] & 1 == 1).fold(0, |a, f| a | 1 << f));
            let placed = give.place(at, middle);
            let (half, round) = stuff.shape(n);
            let mut form = Form { pillow: stuff.pillow(), facets: (stuff == Stuff::Rock).then_some(n), ..Form::new(half, round, 2) };
            form.up = WAYS[world.iter().position(|&w| w == TOP).expect("a face looks up")];
            if block[1] == 2 {
                // Its top sinks in its middle as its column's squashed,
                // and rises as it's stretched, leaning its highlights.
                form.rise = give.rise(middle[0], middle[2]);
                form.dome = 2.0 * form.rise / (half - round);
            }
            blocks.push(Plan { n, block: Some(block), world, pixels: brush.pixels(&placed, round), in_view: brush.in_view(&placed), placed, form, open, own, walls });
        }
        if !whole {
            // Squashed about its bottom, as much wider as keeps its size
            // (it's alone on the top), and carried with the jelly under it.
            let s = give.small();
            let w = 1.0 / s.sqrt();
            let bottom = sub(small, [0.0, 0.5, 0.0]);
            let carried = sub(give.place(Placed::still(bottom), bottom).at, bottom);
            let placed = Placed { m: turn, at: small }.then([[w, 0.0, 0.0], [0.0, s, 0.0], [0.0, 0.0, w]], add(sub(bottom, [w * bottom[0], s * bottom[1], w * bottom[2]]), carried));
            let (half, round) = stuff.shape(26);
            let form = Form { pillow: stuff.pillow(), facets: (stuff == Stuff::Rock).then_some(26), ..Form::new(half, round, 2) };
            blocks.push(Plan { n: 26, block: None, world: [0, 1, 2, 3, 4, 5], pixels: brush.pixels(&placed, round), in_view: brush.in_view(&placed), placed, form, open: [true; 6], own: stuff.small(), walls: 0 });
        }
        cut(&mut blocks, stuff);
        for plan in &blocks {
            let lit = |f: usize| {
                let block = plan.block?;
                let out = WAYS[plan.world[f]];
                let face_middle = add(block.map(|c| c as f32 + 0.5), mul(out, 0.5));
                let outside = face_middle.iter().any(|&c| !(1e-3..=3.0 - 1e-3).contains(&c));
                if outside {
                    self.look(Tile::at(face_middle, out))
                } else {
                    None
                }
            };
            brush.block(plan, lit);
        }
        let drawn = brush.drawn;
        if stuff.sparkles() {
            for (k, &when) in SPARKLES.iter().enumerate() {
                if self.t > when && self.t < when + TWINKLE {
                    sparkle(scene, cam, self.sparkle_at(k, &give, small), (self.t - when) / TWINKLE, h);
                }
            }
        }
        drawn
    }

    /// Where sparkle `k` shines, the small cube's middle at `small` and the
    /// jelly as `give` has it: on the rounded corner of the block it's at,
    /// where that is now: the small cube's, as it hops; jelly's, squashed
    /// and stretched with its column.
    fn sparkle_at(&self, k: usize, give: &Give, small: V3) -> V3 {
        let corner = sparkle_corner(k);
        let rest = corner.map(|c| c.clamp(0.5, 2.5));
        let (half, round) = self.stuff.shape(cell_of(rest));
        let middle = if k == 0 { small } else { rest };
        let on = add(middle, mul(sub(corner, rest).map(f32::signum), half - round + round / 3f32.sqrt()));
        give.place(Placed::still(on), middle).at
    }

    /// The row of stuffs below, the one shown lit, and the keys.
    fn hint(&self) -> Line {
        let names: Vec<&str> = Stuff::ALL.iter().map(|s| s.name()).collect();
        let keys = if self.t >= DONE { "m changes it · space plays it again · q quits" } else { "m changes it · q quits" };
        let text = format!("{}   {keys}", names.join(" · "));
        let i = Stuff::ALL.iter().position(|&s| s == self.stuff).unwrap_or(0);
        let start: usize = names[..i].iter().map(|n| n.chars().count() + 3).sum();
        Line::new(text, WORDS).lit(start..start + names[i].chars().count(), self.stuff.light())
    }
}

impl Default for Cube {
    fn default() -> Self {
        Self::new()
    }
}

/// The eye `t` seconds in, in a picture `wide` times as wide as it's
/// `height` tall, looking at `follow` while it's close. Before the drop
/// it's back over the whole top, so the missing block shows from the
/// start, however narrow the picture; then it's close over the top from
/// the front right, then pulls back and round to the front left, where the
/// notch is the top right; then it goes once round, slowly, to come to
/// rest where it set off.
fn camera_at(t: f32, follow: V3, wide: f32, height: f32) -> Camera {
    const FOV: f32 = 0.72;
    // Far enough that the top's corners, seen from the opening's side,
    // are within the picture's sides.
    let fit = 2.3 / (wide * (FOV * 0.5).tan());
    let open = smooth(LANDS - 0.15, ROLLS + 0.45, t);
    let far_out = fit.max(5.4);
    let near = (lerp([1.6, 2.8, 1.9], follow, open), 0.3, 1.0, far_out + (3.8 - far_out) * open, FOV);
    let k = smooth(PULL.0, PULL.1, t);
    let far = ([1.5, 1.45, 1.5], -0.35, 0.7, 11.0, 0.62);
    let at = |a: f32, b: f32| a + (b - a) * k;
    let round = if t >= TURN.1 { 0.0 } else { -TAU * smooth(TURN.0, TURN.1, t) };
    Camera::orbit(lerp(near.0, far.0, k), at(near.1, far.1) + round, at(near.2, far.2), at(near.3, far.3), at(near.4, far.4), height)
}

/// When the big cube's corners sparkle, catching the light: the first as
/// the small cube clicks into place, at its corner, the rest as the cube
/// goes round, done before it comes to rest; and how long each lasts.
const SPARKLES: [f32; 8] = [FITS + 0.05, 6.3, 7.0, 7.7, 8.4, 9.1, 9.8, 10.5];
const TWINKLE: f32 = 0.4;

/// Where each sparkle shines: the notch's outer corner as the small cube
/// clicks in, then the top's corners in turn, which the eye, always above,
/// always sees.
fn sparkle_corner(k: usize) -> V3 {
    const TOP_CORNERS: [V3; 4] = [[0.0, 3.0, 0.0], [3.0, 3.0, 0.0], [0.0, 3.0, 3.0], [3.0, 3.0, 3.0]];
    if k == 0 {
        [3.0, 3.0, 3.0]
    } else {
        TOP_CORNERS[(k - 1) % TOP_CORNERS.len()]
    }
}

/// A four-pointed star of light at `at`, `life` of the way through its
/// twinkle, a picture `height` pixels tall: it grows, turns a little and
/// goes, over everything, white round a small pale cyan heart. (A haze of
/// see-through color round it only greys the dark glass: a picture can
/// blend over what's behind it, not add light to it.)
fn sparkle(scene: &mut Scene, cam: &Camera, at: V3, life: f32, height: f32) {
    let Some(p) = cam.project(at) else { return };
    let size = height * 0.075 * (PI * life).sin().powf(1.5);
    if size < 0.5 {
        return;
    }
    let spin = 0.6 * life;
    // A thin diamond from the middle out `long` along `angle`, nearer
    // than the cube by `lift`.
    let diamond = |angle: f32, long: f32, wide: f32, lift: f32| {
        let (d, q) = ([angle.cos(), angle.sin()], [-angle.sin(), angle.cos()]);
        [[d[0] * long, d[1] * long], [q[0] * wide, q[1] * wide], [-d[0] * long, -d[1] * long], [-q[0] * wide, -q[1] * wide]].map(|o| [p[0] + o[0], p[1] + o[1], p[2] * lift])
    };
    let heart = Material::plain([0.78, 0.97, 1.0, 1.0]).unlit();
    let light = Material::plain([1.0, 1.0, 1.0, 1.0]).unlit();
    let round: Vec<V3> = (0..8).map(|i| i as f32 * PI / 4.0 + spin).map(|a| [p[0] + a.cos() * size * 0.16, p[1] + a.sin() * size * 0.16, p[2] * 1.09]).collect();
    scene.flat(heart, &round, [0.0, 0.0, 1.0]);
    for (angle, long) in [(spin, 1.0), (spin + FRAC_PI_2, 1.0), (spin + PI / 4.0, 0.45), (spin - PI / 4.0, 0.45)] {
        scene.flat(light, &diamond(angle, size * long, size * 0.09, 1.1), [0.0, 0.0, 1.0]);
    }
}

/// The middle of a square on the big cube's top.
fn top_of((x, z): Square) -> V3 {
    [x as f32 + 0.5, 3.0, z as f32 + 0.5]
}

/// The small cube's middle sitting on a square of the top.
fn above(square: Square) -> V3 {
    add(top_of(square), [0.0, 0.5, 0.0])
}

/// What a roll from one square to the next turns about: the axis, and a
/// point on the edge it tips over.
fn roll_axis(from: Square, to: Square) -> (V3, V3) {
    let d = [(to.0 - from.0) as f32, 0.0, (to.1 - from.1) as f32];
    (cross([0.0, 1.0, 0.0], d), add(top_of(from), mul(d, 0.5)))
}

/// For a face looking `out` on a block square to the world, a way along
/// one of its sides.
fn along(out: V3) -> V3 {
    if out[1].abs() > 0.5 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    }
}

/// The material `k` of the way from `a` to `b`.
fn blend(a: Material, b: Material, k: f32) -> Material {
    if k <= 0.0 {
        return a;
    }
    if k >= 1.0 {
        return b;
    }
    let f = |x: f32, y: f32| x + (y - x) * k;
    Material {
        color: mix(a.color, b.color, k),
        gloss: f(a.gloss, b.gloss),
        metal: f(a.metal, b.metal),
        glow: [0, 1, 2].map(|i| f(a.glow[i], b.glow[i])),
        glow_strength: f(a.glow_strength, b.glow_strength),
        coat: f(a.coat, b.coat),
        coat_gloss: f(a.coat_gloss, b.coat_gloss),
        unlit: false,
    }
}

impl Demo for Cube {
    fn words(&self) -> u16 {
        3
    }

    fn interval(&self) -> Duration {
        if self.t >= DONE && !self.keyed {
            STILL
        } else {
            FRAME
        }
    }

    fn frame(&mut self, dt: f32, w: f32, h: f32) -> Frame {
        self.keyed = false;
        let before = self.t;
        self.t = (self.t + dt).min(DONE);
        // The eye follows the small cube smoothly, not roll by roll.
        let (small, _) = self.small();
        let goal = [small[0], 3.4, small[2]];
        self.follow = if before == 0.0 { goal } else { lerp(self.follow, goal, 1.0 - (-dt * 5.0).exp()) };
        let cam = self.camera(w, h);

        let mut scene = Scene::new(w, h);
        self.draw(&mut scene, &cam, h);

        let shown = NAME.iter().enumerate().map(|(i, &c)| if self.t >= LETTERS + i as f32 * 0.15 { c } else { ' ' }).map(String::from).collect::<Vec<_>>().join("  ");
        let words = vec![Line::new(shown, self.stuff.light()).bold(), Line::new("", WORDS), self.hint()];
        Frame { effects: self.effects(before, &cam, w), show: Show::Scene(scene), words }
    }

    fn key(&mut self, key: KeyCode) {
        let stuff = match key {
            KeyCode::Char(' ') => {
                *self = Self::of(self.stuff);
                return;
            }
            KeyCode::Char('m' | 'M') => self.stuff.next(),
            KeyCode::Char(c @ '1'..='4') => Stuff::ALL[c as usize - '1' as usize],
            _ => return,
        };
        self.stuff = stuff;
        self.keyed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOTTOM: usize = 3;

    #[test]
    fn the_lit_squares_spell_an_f_with_its_bar_at_the_back() {
        let cube = Cube { t: REVEAL - 0.01, ..Cube::new() };
        let lit: Vec<Square> = (0..3).flat_map(|z| (0..3).map(move |x| (x, z))).filter(|&s| cube.lit_at(s).is_some()).collect();
        assert_eq!(lit, vec![(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (0, 2)]);
        let sides: Vec<Tile> = Tile::all().filter(|t| t.face != TOP && cube.lit(*t).is_some()).collect();
        assert!(sides.is_empty(), "the sides wait for the reveal: {sides:?}");
    }

    #[test]
    fn the_small_cube_rolls_square_to_square_and_tips_into_the_notch() {
        for (i, w) in PATH.windows(2).enumerate() {
            assert_eq!((w[0].0 - w[1].0).abs() + (w[0].1 - w[1].1).abs(), 1, "roll {i} goes one square");
        }
        assert_eq!(PATH[ON_TOP], (NOTCH[0], NOTCH[2]), "the last goes over the notch");
        let cube = Cube { t: 100.0, ..Cube::new() };
        let (center, turn) = cube.small();
        assert!(sub(center, IN_NOTCH).iter().all(|d| d.abs() < 1e-4), "it fills the notch: {center:?}");
        // Seven quarter turns along the top and a half turn over the edge
        // leave it square to the world.
        assert!(turn.iter().flatten().all(|v| v.abs() < 1e-6 || (v.abs() - 1.0).abs() < 1e-6), "{turn:?}");
        // Halfway through a roll its center is higher, over the edge.
        let mid = Cube { t: ROLLS + ROLL * 0.5, ..Cube::new() };
        assert!(mid.small().0[1] > 3.6);
        // A quarter of the way over the last edge it stands on it, over
        // the notch, before it drops in.
        let over = Cube { t: TIPS + TIP * 0.5f32.sqrt(), ..Cube::new() };
        assert!(sub(over.small().0, [2.5, 3.5, 2.5]).iter().all(|d| d.abs() < 1e-3), "{:?}", over.small().0);
    }

    #[test]
    fn the_small_cube_never_passes_through_the_big_one() {
        let mut cube = Cube::new();
        while cube.t < DONE {
            cube.t += 0.005;
            let (center, turn) = cube.small();
            // Its corners, the middles of its edges and of its faces.
            for i in (0..27).filter(|&i| i != 13) {
                let local = [i % 3, i / 3 % 3, i / 9].map(|c| (c as f32 - 1.0) * 0.5);
                let p = add(center, apply(turn, local));
                let inside = p.iter().all(|&c| c > 1e-3 && c < 3.0 - 1e-3) && !p.iter().all(|&c| c > 2.0 - 1e-3);
                assert!(!inside, "at {:.3}s {p:?} is inside the big cube", cube.t);
            }
        }
    }

    /// A point's place to a ten-thousandth, to find where pieces meet.
    fn key(a: V3) -> [i32; 3] {
        a.map(|c| (c * 1e4).round() as i32)
    }

    /// The sides of `shell`'s triangles that no other triangle has the
    /// other way round, by where their ends are: where the surface is open.
    fn open_sides(shell: &Shell) -> Vec<(V3, V3)> {
        let mut sides = std::collections::HashMap::new();
        for t in &shell.tris {
            for k in 0..3 {
                let (a, b) = (shell.at[t.v[k] as usize], shell.at[t.v[(k + 1) % 3] as usize]);
                *sides.entry((key(a), key(b))).or_insert(0) += 1;
            }
        }
        let mut open = Vec::new();
        for t in &shell.tris {
            for k in 0..3 {
                let (a, b) = (shell.at[t.v[k] as usize], shell.at[t.v[(k + 1) % 3] as usize]);
                if sides.get(&(key(b), key(a))) != sides.get(&(key(a), key(b))) {
                    open.push((a, b));
                }
            }
        }
        open
    }

    /// How many triangles a corner of a block cut in `steps` takes.
    fn corner_tris(steps: usize) -> usize {
        if fanned(steps) {
            3 * steps
        } else {
            steps * steps
        }
    }

    #[test]
    fn a_rounded_block_is_whole_and_wound_to_face_outward_however_finely_its_cut() {
        // Stone is always cut in two steps, chipped rather than round.
        assert!([1.0, 30.0, 300.0].iter().all(|&p| steps_for(p, Stuff::Rock.most_steps()) == 2));
        for stuff in Stuff::ALL {
            let (half, round) = stuff.shape(5);
            for steps in (1..=6).filter(|&n| stuff != Stuff::Rock || n == 2) {
                let form = Form { pillow: stuff.pillow(), facets: (stuff == Stuff::Rock).then_some(5), ..Form::new(half, round, steps) };
                let shell = shell(&form, [true; 6]);
                // Six faces (stone's a fan to its sides, cut where its
                // edges are), twelve edges in their steps (stone's cut
                // along their length too) and eight corners.
                let (faces, along) = if stuff == Stuff::Rock { (6 * 4 * ALONG, ALONG) } else { (12, 1) };
                assert_eq!(shell.tris.len(), faces + 12 * 2 * steps * along + 8 * corner_tris(steps), "{stuff:?} in {steps}");
                for t in &shell.tris {
                    let [a, b, c] = t.v.map(|i| shell.at[i as usize]);
                    let middle = mul(add(a, add(b, c)), 1.0 / 3.0);
                    assert!(dot(cross(sub(b, a), sub(c, a)), middle) > 0.0, "{stuff:?} in {steps}: a triangle wound inward at {middle:?}");
                }
                for (&p, &n) in shell.at.iter().zip(&shell.normal) {
                    assert!(p.iter().all(|c| c.abs() <= half + 0.02), "{stuff:?}: {p:?} within its half side");
                    if form.facets.is_none() {
                        // On the rounded box: its inner box plus `round` out.
                        let inside = p.map(|c| c.clamp(-(half - round), half - round));
                        assert!((dot(sub(p, inside), sub(p, inside)).sqrt() - round).abs() < 1e-4, "{stuff:?}: {p:?} isn't on the surface");
                        assert!((dot(n, n) - 1.0).abs() < 1e-4 && dot(n, sub(p, inside)) > 0.0, "{stuff:?}: {n:?} faces out");
                    }
                }
                assert!(open_sides(&shell).is_empty(), "{stuff:?} in {steps}: open at {:?}", open_sides(&shell).first());
            }
        }
    }

    #[test]
    fn a_block_shows_only_what_lies_open_and_is_open_only_where_it_meets_its_neighbours() {
        let (half, round) = Stuff::Glass.shape(0);
        let inner = half - round;
        for steps in [2, 3, 4] {
            // A block in the middle of a face: its face, the four rounds
            // into the gaps round it, and its four corners.
            let top = shell(&Form::new(half, round, steps), [false, false, true, false, false, false]);
            assert_eq!(top.tris.len(), 2 + 4 * 2 * steps + 4 * corner_tris(steps));
            assert!(top.at.iter().all(|a| a[1] >= inner - 1e-5), "nothing below its rounded edges");
            // Its only openings are round the bottom of its rounds, where
            // the blocks beside it are.
            for (a, b) in open_sides(&top) {
                assert!((a[1] - inner).abs() < 1e-4 && (b[1] - inner).abs() < 1e-4, "open from {a:?} to {b:?}");
            }
        }
        assert!(half < 0.5 && half > 0.44, "a slim gap between blocks");
    }

    #[test]
    fn a_fan_is_the_triangles_it_was_made_of_in_fewer_corners() {
        let form = Form::new(0.46, 0.14, 4);
        let shell = shell(&form, [true, false, true, false, true, false]);
        let tris: Vec<[u32; 3]> = shell.tris.iter().map(|t| t.v).collect();
        let fans = fans(&tris, shell.at.len());
        let mut out: Vec<[u32; 3]> = fans.iter().flat_map(|f| (1..f.len() - 1).map(move |k| [f[0], f[k], f[k + 1]])).collect();
        // The same triangles, each wound the same way round.
        let turned = |t: [u32; 3]| {
            let k = (0..3).min_by_key(|&k| t[k]).expect("three");
            [t[k], t[(k + 1) % 3], t[(k + 2) % 3]]
        };
        let mut want: Vec<[u32; 3]> = tris.iter().map(|&t| turned(t)).collect();
        out = out.into_iter().map(turned).collect();
        want.sort();
        out.sort();
        assert_eq!(out, want);
        let corners: usize = fans.iter().map(Vec::len).sum();
        assert!(corners * 2 < tris.len() * 3, "{corners} corners for {} triangles", tris.len());
    }

    #[test]
    fn metal_faces_lean_out_from_their_middles_so_a_highlight_crosses_them() {
        let (half, round) = Stuff::Metal.shape(0);
        let form = Form { pillow: Stuff::Metal.pillow(), ..Form::new(half, round, 4) };
        let inner = half - round;
        let middle = form.normal([0.0, half, 0.0], [0.0, 1.0, 0.0]);
        let corner = form.normal([inner, half, inner], [0.0, 1.0, 0.0]);
        assert_eq!(middle, [0.0, 1.0, 0.0]);
        let lean = corner[1].acos().to_degrees();
        assert!((34.0..46.0).contains(&lean), "{lean} degrees at its corner");
        assert!(corner[0] > 0.0 && corner[2] > 0.0, "out toward the corner");
        // And it carries on round the edge without a crease.
        let edge = form.normal([inner, half, 0.0], [0.0, 1.0, 0.0]);
        let round_start = form.normal(add(mul([1.0, 1.0, 0.0], inner), mul([0.0, 1.0, 0.0], round)), [0.0, 1.0, 0.0]);
        assert!(sub(edge, round_start).iter().all(|d| d.abs() < 1e-5));
    }

    #[test]
    fn stone_faces_lean_each_their_own_way_and_their_edges_are_chipped_unevenly() {
        let (half, round) = Stuff::Rock.shape(3);
        let top = |block: usize| shell(&Form { facets: Some(block), ..Form::new(half, round, 2) }, [false, false, true, false, false, false]);
        let (a, b) = (top(3), top(4));
        let face = |s: &Shell| s.tris.iter().filter(|t| t.face.is_some()).map(|t| s.normal[t.v[0] as usize]).collect::<Vec<_>>();
        let (fa, fb) = (face(&a), face(&b));
        assert_eq!(fa.len(), 4 * ALONG);
        // Each face is flat, one facet, leaning a little its own way.
        assert!(fa.windows(2).all(|w| sub(w[0], w[1]).iter().all(|d| d.abs() < 1e-4)), "one facet");
        let lean = fa[0][1].acos().to_degrees();
        assert!(lean < 6.0, "a face leans only a little: {lean}");
        assert!(sub(fa[0], fb[0]).iter().any(|d| d.abs() > 1e-3), "another block's leans otherwise");
        // Its corners are knocked in toward its middle, along each side by
        // as much as its own, some a lot and most a little, so its edges
        // are chipped unevenly.
        let inner = half - round;
        let knocks: Vec<f32> = (0..27)
            .flat_map(|n| {
                let (half, round) = Stuff::Rock.shape(n);
                let form = Form::new(half, round, 2);
                let inner = half - round;
                [[1.0, 1.0], [1.0, -1.0], [-1.0, 1.0], [-1.0, -1.0]].into_iter().flat_map(move |[x, z]| {
                    let p = [x * inner, half, z * inner];
                    let q = chipped(&form, n, p);
                    [(p[0] - q[0]) * x / (KNOCK * round), (p[2] - q[2]) * z / (KNOCK * round)]
                })
            })
            .collect();
        assert!(knocks.iter().all(|&k| (0.0..=1.0 + 1e-4).contains(&k)), "in, by at most KNOCK: {knocks:?}");
        assert!(knocks.iter().filter(|&&k| k < 0.25).count() > knocks.len() / 3 && knocks.iter().any(|&k| k > 0.7), "{knocks:?}");
        assert!(inner > KNOCK * round * 2.0, "never so far that a face's corners meet");
        // And its rounds break into chips that each face their own way.
        let chips: Vec<V3> = a.tris.iter().filter(|t| t.face.is_none()).map(|t| a.normal[t.v[0] as usize]).collect();
        let distinct = chips.iter().enumerate().filter(|(i, n)| chips[..*i].iter().all(|m| sub(**n, *m).iter().any(|d| d.abs() > 1e-3))).count();
        assert!(distinct * 4 > chips.len() * 3, "{distinct} ways among {} chips", chips.len());
    }

    #[test]
    fn every_face_shows_an_upright_f_seen_from_outside() {
        let cube = Cube { t: DONE, ..Cube::new() };
        // From in front of each face, a long way off, the camera's up the
        // sides' up, the back of the top and the front of the bottom.
        let views = [(0.0, 0.0), (FRAC_PI_2, 0.0), (PI, 0.0), (-FRAC_PI_2, 0.0), (0.0, 1.5), (0.0, -1.5)];
        for (yaw, pitch) in views {
            let cam = Camera::orbit(MIDDLE, yaw, pitch, 40.0, 0.3, 600.0);
            let toward = [yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos()];
            let face: Vec<(Tile, V3)> = Tile::all().filter(|t| dot(t.out(), toward) > 0.9).map(|t| (t, cam.project(t.middle()).expect("in view"))).collect();
            assert_eq!(face.len(), 9);
            let (left, top) = (face.iter().map(|f| f.1[0]).fold(f32::INFINITY, f32::min), face.iter().map(|f| f.1[1]).fold(f32::NEG_INFINITY, f32::max));
            let step = face.iter().map(|f| f.1[0]).fold(f32::NEG_INFINITY, f32::max) - left;
            let mut lit: Vec<(i32, i32)> = face.iter().filter(|f| cube.lit(f.0).is_some()).map(|f| (((f.1[0] - left) / step * 2.0).round() as i32, ((top - f.1[1]) / step * 2.0).round() as i32)).collect();
            lit.sort();
            assert_eq!(lit, vec![(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (2, 0)], "the F seen from {yaw:.2}, {pitch:.2}: bar at the top, stem at the left");
        }
    }

    #[test]
    fn at_rest_every_face_of_the_f_is_lit_and_settled_whatever_its_made_of() {
        for stuff in Stuff::ALL {
            let cube = Cube { t: DONE, ..Cube::of(stuff) };
            for t in Tile::all() {
                assert_eq!(cube.look(t), t.in_f().then_some(0.0), "{stuff:?}: {t:?}");
            }
        }
    }

    #[test]
    fn the_light_runs_from_the_top_down_the_sides_rather_than_coming_on_at_once() {
        let cube = Cube::new();
        let when = |tile: Tile| Cube { t: 100.0, ..Cube::new() }.lit(tile).expect("in an F");
        let side: Vec<Tile> = Tile::all().filter(|t| t.face != TOP && t.in_f()).collect();
        assert_eq!(side.len(), 30);
        for t in &side {
            let below = Tile { row: t.row + 1, ..*t };
            if t.face != BOTTOM && below.row < 3 && below.in_f() {
                assert!(when(below) > when(*t), "{below:?} lights after {t:?} above it");
            }
            assert!(when(*t) > REVEAL && when(*t) < TURN.1, "{t:?}");
        }
        // The bottom's F lights last, from the sides'.
        let first = |face: usize| side.iter().filter(|t| t.face == face).map(|t| when(*t)).fold(f32::INFINITY, f32::min);
        assert!([0, 1, 4, 5].iter().all(|&face| first(face) < first(BOTTOM)));
        // Partway, some of the sides' F is lit and some isn't.
        let partway = Cube { t: REVEAL + SPREAD * 2.5, ..cube };
        let lit = side.iter().filter(|t| partway.lit(**t).is_some()).count();
        assert!(lit > 0 && lit < side.len(), "{lit} lit");
    }

    #[test]
    fn a_square_coming_on_is_bright_but_never_white_nor_glows_more_in_the_light() {
        for stuff in Stuff::ALL {
            for light in [0.0, 0.5, 1.0] {
                let glowing = stuff.lit(1.0, light);
                for m in glowing.all() {
                    assert!(m.glow.iter().fold(f32::INFINITY, |a, &b| a.min(b)) < 0.7, "{stuff:?} still colored: {:?}", m.glow);
                    assert!(m.glow_strength <= 1.3, "{stuff:?}: {}", m.glow_strength);
                    assert_eq!(m.color[3], 1.0, "{stuff:?}'s light is solid, drawn before any glass");
                }
                assert!(stuff.lit(0.0, light).heart.glow_strength < glowing.heart.glow_strength, "{stuff:?} settles");
            }
            assert!(stuff.lit(0.0, 1.0).heart.glow_strength <= stuff.lit(0.0, 0.0).heart.glow_strength, "{stuff:?} glows no more in the light");
        }
        // Polished metal reflects the light falling on it, so its own gives
        // way there; the see-through stuff's heart has a dimmer halo round
        // it, and stone's embers a crust.
        assert!(Stuff::Metal.lit(0.0, 1.0).heart.glow_strength < Stuff::Metal.lit(0.0, 0.0).heart.glow_strength);
        for stuff in [Stuff::Glass, Stuff::Jelly, Stuff::Rock] {
            let glowing = stuff.lit(0.0, 0.0);
            let halo = glowing.halo.expect("a halo");
            assert!(halo.glow_strength < glowing.heart.glow_strength, "{stuff:?}");
        }
        assert!(Stuff::Rock.lit(0.0, 0.0).crust.is_some());
        let cube = Cube { t: REVEAL + 2.0, ..Cube::new() };
        assert_eq!(cube.look(Tile { face: 4, col: 0, row: 0 }), Some(0.0), "settled once it's lit");
    }

    #[test]
    fn a_see_through_lights_layers_are_rounded_and_sit_inside_the_block_under_its_face() {
        let square = rounded_square(1.0, 0.45, ROUNDED);
        assert_eq!(square.len(), 4 * ROUNDED);
        const { assert!(ROUNDED >= 5, "a corner in four parts at least, or it reads as cut") };
        assert!(square.iter().all(|p| p[0].abs() <= 1.0 + 1e-6 && p[1].abs() <= 1.0 + 1e-6));
        let twice_area: f32 = (0..square.len()).map(|i| (square[i], square[(i + 1) % square.len()])).map(|(a, b)| a[0] * b[1] - b[0] * a[1]).sum();
        assert!(twice_area > 0.0, "counter-clockwise");
        for stuff in [Stuff::Glass, Stuff::Jelly] {
            let (half, round) = stuff.shape(0);
            let inner = half - round;
            let form = Form::new(half, round, 4);
            let [(halo, halo_sink), (heart, heart_sink)] = stuff.light_layers(&form);
            assert!(heart < halo && heart_sink < halo_sink, "{stuff:?}: the heart within the halo, nearer the face");
            // The halo's farthest point, its corner, sunk deepest, is inside
            // the rounded block.
            let r = halo * 0.45;
            let corner = halo - r + r * (PI / 4.0).cos();
            let p = [corner, half - halo_sink, corner];
            let inside = p.map(|c| c.clamp(-inner, inner));
            assert!(dot(sub(p, inside), sub(p, inside)).sqrt() < round, "{stuff:?}'s halo pokes out");
        }
        // Jelly's light keeps to its face's flat part, just under it, so a
        // line of sight through the face at a slant meets the light rather
        // than the magenta behind it.
        let (half, round) = Stuff::Jelly.shape(0);
        let [(halo, sink), _] = Stuff::Jelly.light_layers(&Form::new(half, round, 4));
        assert!(halo <= half - round + 1e-6 && sink <= 0.05, "{halo} across, {sink} under");
        // Its face sinks as it wobbles, and the light with it (from the
        // lowest the face is), so it never shows through the face.
        let dip = (0..(QUIET * 100.0) as usize).map(|i| Give::new(Stuff::Jelly, i as f32 * 0.01)).flat_map(|g| (0..9).map(move |c| g.rise(0.5 + (c % 3) as f32, 0.5 + (c / 3) as f32))).fold(0.0f32, f32::min);
        assert!(dip < -0.02, "{dip}");
    }

    #[test]
    fn a_face_in_the_light_is_more_in_it_than_one_in_shadow() {
        let cam = camera_at(DONE, [2.5, 3.4, 2.5], 1200.0 / 640.0, 640.0);
        let (top, front) = (in_light(&cam, [0.0, 1.0, 0.0]), in_light(&cam, [0.0, 0.0, 1.0]));
        assert!(top > 0.7 && front < 0.3, "the top {top}, the front {front}");
    }

    #[test]
    fn once_in_the_small_cube_lights_with_the_faces_it_fills() {
        let cube = Cube { t: DONE, ..Cube::new() };
        let lit = |out: V3| cube.look(Tile::at(add(IN_NOTCH, mul(out, 0.5)), out)).is_some();
        assert!(lit([0.0, 0.0, 1.0]), "the front's bar runs through it");
        assert!(lit([1.0, 0.0, 0.0]), "so does the right's");
        assert!(!lit([0.0, 1.0, 0.0]), "the top's F doesn't reach its corner");
    }

    #[test]
    fn before_the_drop_the_whole_top_and_its_missing_block_are_in_view() {
        // The top's corners and the notch's floor, wide and narrow.
        let points = [[0.0, 3.0, 0.0], [3.0, 3.0, 0.0], [0.0, 3.0, 3.0], [3.0, 3.0, 3.0], [2.0, 3.0, 3.0], [3.0, 3.0, 2.0], [2.0, 2.0, 2.0], [3.0, 2.0, 3.0]];
        for (w, h) in [(1200.0, 640.0), (640.0, 800.0), (400.0, 700.0)] {
            for t in [0.0, FALL, LANDS - 0.2] {
                let cam = camera_at(t, above(START), w / h, h);
                for p in points {
                    let at = cam.project(p).expect("in front");
                    assert!(at[0].abs() < w * 0.5 && at[1].abs() < h * 0.5, "{p:?} at {t:.2}s in {w} x {h}: {at:?}");
                }
            }
        }
    }

    #[test]
    fn its_landings_ripple_the_fit_thuds_and_the_reveal_bursts_once() {
        let mut cube = Cube::new();
        let mut each = Vec::new();
        let mut thuds = Vec::new();
        for _ in 0..(DONE * 60.0) as usize + 10 {
            let frame = cube.frame(1.0 / 60.0, 1200.0, 700.0);
            for f in frame.effects {
                if f.kind == Kind::Ripple && f.sound == flow_sdk::fx::FxSound::Named(Sound::Thud) {
                    thuds.push(cube.t);
                }
                each.push((cube.t, f.kind));
            }
        }
        assert_eq!(each.iter().filter(|e| e.1 == Kind::Burst).count(), 1);
        // The drop, a roll onto each square, the fit and the reveal.
        assert_eq!(each.iter().filter(|e| e.1 == Kind::Ripple).count(), 1 + PATH.len() + 1);
        // The fit thuds as it settles into the notch, within a frame of it.
        assert!(thuds.iter().any(|t| *t >= FITS && *t < FITS + 1.0 / 60.0 + 1e-4), "no thud at the fit ({FITS:.2}s): {thuds:?}");
        // A few a second at most, within Flow's allowance.
        for (t, _) in &each {
            assert!(each.iter().filter(|e| e.0 >= *t && e.0 < t + 1.0).count() <= 4, "too many after {t:.2}s");
        }
        // Whatever it's made of, the burst is its F's light.
        for stuff in Stuff::ALL {
            let mut cube = Cube { t: REVEAL - 0.01, ..Cube::of(stuff) };
            let burst = cube.frame(0.02, 1200.0, 700.0).effects.into_iter().find(|f| f.kind == Kind::Burst).expect("a burst");
            assert_eq!(burst.color, Some(stuff.light()), "{stuff:?}");
        }
    }

    #[test]
    fn each_sparkle_is_on_a_corner_the_eye_sees_and_all_are_done_before_it_rests() {
        for (k, &when) in SPARKLES.iter().enumerate() {
            let corner = sparkle_corner(k);
            for t in [when, when + TWINKLE * 0.5, when + TWINKLE] {
                let cam = camera_at(t, [2.5, 3.4, 2.5], 1200.0 / 640.0, 640.0);
                // A corner is seen when a face it's on shows its front.
                let seen_on = FACES.iter().filter(|(out, _)| dot(*out, sub(corner, MIDDLE)) > 0.0).any(|&(out, up)| {
                    let right = cross(mul(out, -1.0), up);
                    let at = |a: f32, b: f32| add(MIDDLE, add(mul(out, 1.5), add(mul(right, a), mul(up, b))));
                    let shown: Vec<V3> = [at(-1.5, -1.5), at(1.5, -1.5), at(1.5, 1.5), at(-1.5, 1.5)].iter().map(|&p| cam.project(p).expect("in front")).collect();
                    (0..4).map(|i| (shown[i], shown[(i + 1) % 4])).map(|(a, b)| a[0] * b[1] - b[0] * a[1]).sum::<f32>() > 0.0
                });
                assert!(seen_on, "sparkle {k} at {t:.2}s on {corner:?}");
            }
            assert!(when + TWINKLE < DONE);
        }
    }

    #[test]
    fn the_sparkle_at_the_fit_is_on_the_small_cubes_corner_as_the_jelly_has_it() {
        for stuff in [Stuff::Glass, Stuff::Jelly] {
            for t in [SPARKLES[0] + 0.03, FITS + 0.08, FITS + 0.25] {
                let cube = Cube { t, ..Cube::of(stuff) };
                let (small, turn) = cube.small();
                let give = Give::new(stuff, t);
                let at = cube.sparkle_at(0, &give, small);
                // The notch's block as it's drawn, and its corner nearest.
                let placed = give.place(Placed { m: turn, at: small }, small);
                let (half, round) = stuff.shape(26);
                let block = shell(&Form::new(half, round, 4), [true; 6]);
                let nearest = block.at.iter().map(|&p| sub(placed.point(p), at)).map(|d| dot(d, d).sqrt()).fold(f32::INFINITY, f32::min);
                assert!(nearest < 1e-3, "{stuff:?} at {t:.2}s: {nearest} off its corner");
                let still = cube.sparkle_at(0, &Give::new(Stuff::Glass, t), small);
                let moved = sub(at, still);
                if stuff == Stuff::Jelly && t > FITS + 0.05 {
                    assert!(dot(moved, moved).sqrt() > 0.05, "{t:.2}s: the jelly carries it");
                }
            }
        }
    }

    #[test]
    fn once_still_its_frames_are_the_same_whatever_its_made_of() {
        for stuff in Stuff::ALL {
            let mut cube = Cube { t: DONE, ..Cube::of(stuff) };
            cube.follow = [2.5, 3.4, 2.5];
            let a = cube.frame(0.016, 1200.0, 700.0).scene().expect("a scene").glb((1.0, 1.0));
            let b = cube.frame(0.016, 1200.0, 700.0).scene().expect("a scene").glb((1.0, 1.0));
            assert!(a == b, "{stuff:?}: nothing moves, so nothing need be sent");
        }
    }

    #[test]
    fn m_and_the_digits_change_what_its_made_of_without_losing_its_place() {
        let mut cube = Cube::new();
        for _ in 0..200 {
            cube.frame(1.0 / 60.0, 1200.0, 700.0);
        }
        let (t, follow) = (cube.t, cube.follow);
        cube.key(KeyCode::Char('m'));
        assert_eq!((cube.stuff, cube.t, cube.follow), (Stuff::Metal, t, follow), "m goes on to the next, where it was");
        cube.key(KeyCode::Char('m'));
        cube.key(KeyCode::Char('m'));
        cube.key(KeyCode::Char('m'));
        assert_eq!(cube.stuff, Stuff::Glass, "and round again");
        for (key, stuff) in [('3', Stuff::Rock), ('4', Stuff::Jelly), ('2', Stuff::Metal), ('1', Stuff::Glass)] {
            cube.key(KeyCode::Char(key));
            assert_eq!((cube.stuff, cube.t), (stuff, t));
        }
        cube.key(KeyCode::Char('3'));
        cube.key(KeyCode::Char(' '));
        assert_eq!((cube.stuff, cube.t), (Stuff::Rock, 0.0), "space plays it again in rock");
    }

    #[test]
    fn once_still_it_draws_ten_times_a_second_but_at_once_after_a_key() {
        let mut cube = Cube::new();
        assert_eq!(cube.interval(), FRAME, "while it plays");
        cube.t = DONE;
        assert_eq!(cube.interval(), STILL);
        cube.key(KeyCode::Char('m'));
        assert_eq!(cube.interval(), FRAME, "the next frame comes at once");
        cube.frame(0.016, 1200.0, 700.0);
        assert_eq!(cube.interval(), STILL);
        cube.key(KeyCode::Char('x'));
        assert_eq!(cube.interval(), STILL, "a key it doesn't know changes nothing");
        cube.key(KeyCode::Char(' '));
        assert_eq!((cube.t, cube.interval()), (0.0, FRAME), "and space plays it again");
    }

    #[test]
    fn the_words_name_each_stuff_and_light_the_one_shown() {
        for stuff in Stuff::ALL {
            let mut cube = Cube::of(stuff);
            let line = cube.frame(0.016, 1200.0, 700.0).words.pop().expect("a hint");
            assert!(line.text.starts_with("glass · metal · rock · jelly"), "{}", line.text);
            let (range, color) = line.lit.clone().expect("one lit");
            assert_eq!(line.text.chars().skip(range.start).take(range.len()).collect::<String>(), stuff.name());
            assert_eq!(color, stuff.light());
        }
        let done = Cube { t: DONE, ..Cube::new() };
        assert!(done.hint().text.contains("space plays it again"));
    }

    /// The most see-through triangles a frame of a see-through cube may
    /// have: Flow cuts them where they cross and sorts them as it loads
    /// each frame, and cuts no more than 2,048 a frame for a model that
    /// moves by itself, which the stage's two corners bring these to. A
    /// play has about 2,040 at most in glass and jelly, at the shapes of
    /// picture tried; on a Mac, Flow loads such a frame in about a
    /// millisecond.
    const SEE_THROUGH_MOST: usize = 2046;

    /// Every `step` seconds of a play of `stuff` in a picture `w` × `h`,
    /// the eye where it would be: what each frame drew.
    fn play(stuff: Stuff, (w, h): (f32, f32), step: f32) -> Vec<(f32, Drawn)> {
        (0..=(DONE / step) as usize)
            .map(|i| {
                let mut cube = Cube { t: (i as f32 * step).min(DONE), ..Cube::of(stuff) };
                let small = cube.small().0;
                cube.follow = [small[0], 3.4, small[2]];
                (cube.t, cube.draw(&mut Scene::new(w, h), &cube.camera(w, h), h))
            })
            .collect()
    }

    #[test]
    fn a_see_through_cube_keeps_its_see_through_triangles_few_and_the_solid_ones_none() {
        // Every twentieth of a second of a play at a wide picture; and
        // every quarter at a narrow one.
        for stuff in Stuff::ALL {
            for (w, h, step) in [(1200.0, 700.0, 0.05), (500.0, 700.0, 0.25)] {
                let drawn = play(stuff, (w, h), step);
                let most = drawn.iter().map(|d| d.1.see_through).max().unwrap_or(0);
                let tinted = drawn.iter().map(|d| d.1.tinted).max().unwrap_or(0);
                if stuff.see_through() {
                    assert!(most > 0 && most <= SEE_THROUGH_MOST, "{stuff:?} at {w} x {h}: {most} see-through triangles in a frame");
                    // Flow sorts them farthest first and draws them in runs
                    // of one material: the glass, broken only either side of
                    // each tinted triangle, and Flow's own corners.
                    assert!(tinted > 0 && 2 * tinted + 3 < 256, "{stuff:?}: {tinted} tinted triangles, too many runs for Flow");
                } else {
                    assert_eq!((most, tinted), (0, 0), "{stuff:?} is solid");
                }
            }
        }
    }

    #[test]
    fn its_blocks_are_rounder_close_up_and_far_sides_coarser_but_never_crossing_the_near() {
        let wide = (1200.0, 700.0);
        for stuff in Stuff::ALL {
            let steps_at = |t: f32| {
                let mut cube = Cube { t, ..Cube::of(stuff) };
                let small = cube.small().0;
                cube.follow = [small[0], 3.4, small[2]];
                let cam = cube.camera(wide.0, wide.1);
                let mut scene = Scene::new(wide.0, wide.1);
                let brush = Brush { scene: &mut scene, cam: &cam, stuff, drawn: Drawn::default() };
                let (half, round) = stuff.shape(24);
                let placed = Placed::still([0.5, 2.5, 2.5]);
                let mut plan = Plan { n: 24, block: Some([0, 2, 2]), world: [0, 1, 2, 3, 4, 5], pixels: brush.pixels(&placed, round), in_view: brush.in_view(&placed), placed, form: Form::new(half, round, 2), open: [false, true, true, false, true, false], own: stuff.body(24), walls: 0 };
                cut(std::slice::from_mut(&mut plan), stuff);
                plan.form.steps
            };
            // The front left of the top, close up as the small cube rolls
            // and far off at rest. (Stone's are chipped, two steps always.)
            // (Glass's most, four, is what its blocks at rest are cut in
            // too: three would cost as much.)
            let (near, far) = (steps_at(ROLLS + 2.5 * STEP), steps_at(DONE));
            assert!(near <= stuff.most_steps() && far >= 2);
            assert!(near > far || (stuff == Stuff::Glass && near == 4) || stuff == Stuff::Rock, "{stuff:?}: {near} close up, {far} at rest");
        }
        // A see-through block's far side is cut from points of its near
        // side, so the two never cross (Flow would cut where they do); and
        // never in a step, which shows as a smaller block inside the round
        // one.
        for n in [2, 3, 4, 6] {
            let far = far_steps(n);
            assert!(n % far == 0 && far >= 2 && (far == n || 2 * far == n), "{n}: {far}");
            let (half, round) = Stuff::Jelly.shape(0);
            let near = shell(&Form::new(half, round, n), [true; 6]);
            let far = shell(&Form { near: Some(n), ..Form::new(half, round, far) }, [true; 6]);
            let points: Vec<[i32; 3]> = near.at.iter().map(|&p| key(p)).collect();
            assert!(far.at.iter().all(|&p| points.contains(&key(p))), "{n}: a point of the far side that isn't the near side's");
        }
        for stuff in [Stuff::Glass, Stuff::Jelly] {
            let mut cube = Cube { t: ROLLS + 2.5 * STEP, ..Cube::of(stuff) };
            cube.follow = [cube.small().0[0], 3.4, cube.small().0[2]];
            let cam = cube.camera(wide.0, wide.1);
            let mut scene = Scene::new(wide.0, wide.1);
            let brush = Brush { scene: &mut scene, cam: &cam, stuff, drawn: Drawn::default() };
            let mut plans: Vec<Plan> = (0..27)
                .filter(|&n| n != 13 && n != 26)
                .map(|n| {
                    let block = [n as i32 % 3, n as i32 / 3 % 3, n as i32 / 9];
                    let placed = Placed::still(block.map(|c| c as f32 + 0.5));
                    let (half, round) = stuff.shape(n);
                    Plan { n, block: Some(block), world: [0, 1, 2, 3, 4, 5], pixels: brush.pixels(&placed, round), in_view: brush.in_view(&placed), placed, form: Form::new(half, round, 2), open: [true; 6], own: stuff.body(n), walls: 0 }
                })
                .collect();
            cut(&mut plans, stuff);
            assert!(plans.iter().all(|p| p.form.steps != 5), "{stuff:?}: five steps would leave its far side one chamfer");
            assert!(plans.iter().all(|p| p.form.steps != 3), "{stuff:?}: three steps cost as much as four");
        }
        // Each step a see-through block is made coarser by saves triangles:
        // six to four, then four to two, whatever of it is open.
        for open in (1..64u8).map(|bits| std::array::from_fn(|f| bits >> f & 1 == 1)) {
            let costs = [6, 4, 2].map(|steps| cost(open, steps));
            assert!(costs[0] > costs[1] && costs[1] > costs[2], "{open:?}: {costs:?}");
            assert_eq!(cost(open, 3), cost(open, 4), "{open:?}: three is no cheaper than four");
        }
    }

    #[test]
    fn the_notch_shows_its_floor_and_walls_till_the_small_cube_fills_it() {
        // Before the fit the notch itself isn't drawn, nor ever the middle
        // block, and the three blocks round the notch show it their floor
        // and walls, open and marked.
        assert!(faces_of(NOTCH, false).is_none());
        assert!(faces_of([1, 1, 1], false).is_none() && faces_of([1, 1, 1], true).is_none());
        let round_it = [([2, 1, 2], 2), ([1, 2, 2], 0), ([2, 2, 1], 4)];
        for (block, way) in round_it {
            let (open, walls) = faces_of(block, false).expect("drawn");
            assert!(open[way] && walls == 1 << way, "{block:?}: {open:?}, walls {walls:b}");
        }
        let faces = |whole: bool| (0..27).filter_map(|n| faces_of(cell(n), whole)).fold((0, 0), |(o, w), (open, walls)| (o + open.iter().filter(|&&o| o).count(), w + walls.count_ones()));
        // The big cube's 54 outer faces but the notch's three, and its
        // three walls.
        assert_eq!(faces(false), (51 + 3, 3));
        // Once it's filled, its block shows its three outer faces, the
        // walls are closed, and nothing is a wall.
        assert_eq!(faces_of(NOTCH, true), Some(([true, false, true, false, true, false], 0)));
        for (block, way) in round_it {
            assert!(!faces_of(block, true).expect("drawn").0[way], "{block:?} closed once it's filled");
        }
        assert_eq!(faces(true), (54, 0));
    }

    #[test]
    fn a_lit_stone_face_is_ash_crust_cracked_over_a_glow_hot_in_the_middle_of_each_crack() {
        let glowing = Stuff::Rock.lit(0.0, 0.0);
        let (warm, ash, hot, deep) = (glowing.crust.expect("warm edges"), glowing.ash.expect("ash"), glowing.heart, glowing.halo.expect("deep red"));
        let bright = |m: Material| m.glow.iter().sum::<f32>() * m.glow_strength;
        assert!(warm.color[..3].iter().all(|&c| c < 0.15) && bright(warm) < 0.25 * bright(deep), "the plates' edges are dark, warmed a little");
        assert!(warm.glow[0] > 2.0 * warm.glow[1], "and red");
        assert!(ash.color[..3].iter().all(|&c| c < 0.3) && bright(ash) < bright(warm), "the ash is dark, warm only at its edges");
        assert!(hot.glow[1] > 0.6 * hot.glow[0] && deep.glow[1] < 0.3 * deep.glow[0], "a yellow core, deep red round it");
        assert!(bright(hot) > bright(deep));
        // Each lit face of each block, a pattern of its own.
        let mut patterns = Vec::new();
        for n in 0..27 {
            let (half, round) = Stuff::Rock.shape(n);
            let form = Form { facets: Some(n), ..Form::new(half, round, 2) };
            for f in 0..6 {
                let edge = face_edge(&form, n, f);
                let face = ember_face(n * 6 + f, &edge);
                let area = twice_area(&edge);
                let of = |kind: fn(Ember) -> bool| face.pieces.iter().filter(|p| kind(p.0)).map(|p| twice_area(&p.1)).sum::<f32>();
                for (kind, piece) in &face.pieces {
                    assert!(twice_area(piece) > 0.0, "{n}, {f}: {kind:?} goes counter-clockwise");
                    assert!(fan_pieces([0.0; 2], piece).len() == 1, "{n}, {f}: {kind:?} is convex");
                }
                // The glow reaches past the face's edge, under the rounds,
                // so nothing between them is open; the plates keep to it,
                // and between them, the cracks.
                let below = of(|k| k == Ember::Deep);
                assert!(below > area * 1.02 && below < area * 1.2, "{n}, {f}: the glow under it, {below} of {area}");
                let edge_in = |p: [f32; 2]| face.pieces.iter().filter(|q| q.0 == Ember::Deep).any(|q| within(&[p, [p[0] + 1e-4, p[1]], [p[0], p[1] + 1e-4]], &q.1).len() == 3);
                assert!(edge.iter().all(|&p| edge_in(p)), "{n}, {f}: the glow reaches the face's edge");
                let (plates, ashes) = (of(|k| matches!(k, Ember::Warm(_))), of(|k| matches!(k, Ember::Ash(_))));
                assert!(plates < area * 0.98 && plates > area * 0.65, "{n}, {f}: plates over {} of it", plates / area);
                assert!(ashes < plates * 0.95 && ashes > plates * 0.5, "{n}, {f}: their warm edges, {} of them", 1.0 - ashes / plates);
                assert!((3..=5).contains(&face.plates.len()));
                patterns.push((face.plates.len(), (plates * 1e3) as i32, face.pieces.len()));
            }
        }
        patterns.sort();
        patterns.dedup();
        assert!(patterns.len() > 27 * 6 * 9 / 10, "{} patterns of {}", patterns.len(), 27 * 6);
    }

    #[test]
    fn the_gaps_between_solid_blocks_close_on_a_dark_core_that_leaves_the_notch_open_till_its_filled() {
        let at = |stuff: Stuff, t: f32| {
            let mut cube = Cube { t, ..Cube::of(stuff) };
            cube.follow = [2.5, 3.4, 2.5];
            // Far off, all of it in view.
            let cam = Camera::orbit(MIDDLE, 0.6, 0.6, 30.0, 0.3, 700.0);
            cube.draw(&mut Scene::new(1200.0, 700.0), &cam, 700.0).core
        };
        for stuff in Stuff::ALL {
            if stuff.see_through() {
                assert!(stuff.core().is_none());
                assert_eq!(at(stuff, 0.5), 0, "{stuff:?}'s gaps are seen through");
            } else {
                assert!(stuff.core().is_some_and(|m| m.color[3] == 1.0 && m.color[..3].iter().all(|&c| c < 0.1)), "{stuff:?}'s core is dark and solid");
                // Three squares and, round the notch, three squares with
                // its corner left out; once it's filled, six squares.
                assert_eq!(at(stuff, 0.5), 3 * 2 + 3 * 4, "{stuff:?} before the fit");
                assert_eq!(at(stuff, DONE), 6 * 2, "{stuff:?} once it's whole");
            }
        }
    }

    #[test]
    fn a_see_through_cubes_blocks_are_one_glass_near_and_far_and_its_light_solid_under_a_tint() {
        for stuff in Stuff::ALL.into_iter().filter(|s| s.see_through()) {
            assert_eq!(stuff.small(), stuff.body(0));
            assert!((0..27).all(|n| stuff.body(n) == stuff.body(0)));
            assert_eq!(stuff.inner(), Some(stuff.body(0)));
            assert!(stuff.body(0).color[3] < 1.0 && stuff.lit_cover().is_some_and(|m| m.color[3] < 1.0));
            // The notch's walls and floor in shadow, darker than the rest and
            // the floor darkest; solid, so they don't break Flow's runs of
            // glass.
            let (wall, floor) = (stuff.notch(false).expect("walls"), stuff.notch(true).expect("a floor"));
            let light = |m: Material| m.color[0] + m.color[1] + m.color[2];
            assert!(wall.color[3] == 1.0 && floor.color[3] == 1.0, "{stuff:?}: solid");
            assert!(light(floor) < light(wall) && light(wall) < 0.6 * light(stuff.body(0)), "{stuff:?}: in shadow");
            assert_eq!(stuff.lit(0.5, 0.5).heart.color[3], 1.0);
        }
    }

    #[test]
    fn the_jelly_wobbles_as_it_is_struck_and_is_still_before_it_rests() {
        let at = |t: f32| Give::new(Stuff::Jelly, t);
        // The column the small cube lands on squashes just after it does,
        // by a good part of its height.
        let [x, _, z] = top_of(START);
        let struck = (0..20).map(|i| at(LANDS + i as f32 * 0.01).squash(x, z)).fold(1.0f32, f32::min);
        assert!(struck < 0.88, "{struck}");
        let small = (0..20).map(|i| at(LANDS + i as f32 * 0.01).small()).fold(1.0f32, f32::min);
        assert!(small < struck, "the small cube squashes most: {small}");
        // A column far off feels it later and less.
        let far = (0..60).map(|i| at(LANDS + i as f32 * 0.01).squash(2.5, 0.5)).fold(1.0f32, f32::min);
        assert!(far > struck && far < 1.0, "{far}");
        // The whole sways, its top moving away from the blow, enough to see
        // from above; and back.
        let sway = |t: f32| at(t).lean;
        let most = (0..60).map(|i| sway(LANDS + i as f32 * 0.01)).max_by(|a, b| a[0].hypot(a[1]).total_cmp(&b[0].hypot(b[1]))).expect("some");
        assert!(most[0].hypot(most[1]) > 0.08, "{most:?}");
        assert!(most[0] > 0.0 && most[1] < 0.0, "away from the front left: {most:?}");
        // Its top's middle sinks as it's squashed.
        assert!(at(LANDS + 0.06).rise(x, z) < -0.02);
        // Placed, a block's column is squashed from the bottom, so the
        // gaps between layers never close.
        let give = at(LANDS + 0.06);
        let s = give.squash(0.5, 2.5);
        let placed = give.place(Placed::still([0.5, 2.5, 2.5]), [0.5, 2.5, 2.5]);
        assert!((placed.point([0.0, -0.5, 0.0])[1] - 2.0 * s).abs() < 1e-4);
        // Still from `QUIET` on, before it comes to rest.
        for t in [QUIET, (QUIET + DONE) * 0.5, DONE] {
            let give = at(t);
            assert!(!give.on && give.lean == [0.0; 2] && give.small() == 1.0);
            for n in 0..27 {
                let middle = [n % 3, n / 3 % 3, n / 9].map(|c| c as f32 + 0.5);
                assert_eq!(give.squash(middle[0], middle[2]), 1.0);
                let placed = give.place(Placed::still(middle), middle);
                assert_eq!((placed.m, placed.at), (IDENTITY, middle));
            }
        }
        assert!(QUIET < TURN.1 && QUIET < LETTERS);
        // Never so wide that a block reaches its neighbour beside it: two
        // half sides of 0.45 have a tenth between them. (Its height gives
        // with its column's, from the bottom, so the layers never meet;
        // and the whole leans as one.)
        let (half, _) = Stuff::Jelly.shape(0);
        let mut widest = 0.0f32;
        for i in 0..(DONE * 100.0) as usize {
            let give = at(i as f32 * 0.01);
            for n in 0..27 {
                let middle = [n % 3, n / 3 % 3, n / 9].map(|c| c as f32 + 0.5);
                widest = widest.max(give.squash(middle[0], middle[2]).powf(-WIDEN));
            }
        }
        assert!(half * widest < 0.495, "widened to {widest}");
        // Other stuff never gives.
        for stuff in [Stuff::Glass, Stuff::Metal, Stuff::Rock] {
            let give = Give::new(stuff, LANDS + 0.04);
            assert!(!give.on && give.squash(x, z) == 1.0 && give.small() == 1.0);
        }
    }

    #[test]
    fn stone_differs_a_little_from_block_to_block() {
        let colors: Vec<[f32; 4]> = (0..27).map(|n| Stuff::Rock.body(n).color).collect();
        let rounds: Vec<f32> = (0..27).map(|n| Stuff::Rock.shape(n).1).collect();
        assert!(rounds.iter().any(|&r| r < 0.065) && rounds.iter().any(|&r| r > 0.07), "chipped more and less: {rounds:?}");
        let spread = |k: usize| colors.iter().map(|c| c[k]).fold(f32::NEG_INFINITY, f32::max) - colors.iter().map(|c| c[k]).fold(f32::INFINITY, f32::min);
        assert!((0..3).all(|k| spread(k) > 0.02 && spread(k) < 0.2), "{:?}", (0..3).map(spread).collect::<Vec<_>>());
    }
}
