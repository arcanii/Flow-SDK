//! Vector: glowing lines, as the vector monitors of the arcades drew them
//! around 1980, a beam tracing each line on a dark tube rather than a
//! raster's rows. The eye flies low over a grid toward hills under a
//! crescent moon, a volcano among them erupting now and then, and the word
//! FLOW turns ahead of it in wireframe. It's a 3D scene sent with the Ratty
//! graphics protocol as the other scenes are, each line a hot core inside
//! two halos, over a tube's face that darkens the glass. A tube's
//! afterglow and curve are Flow's CRT's, which only its Settings turn on,
//! so the words below say so. The volcano strikes the liquid with
//! lightning as it erupts, a click bursts the word apart (and the liquid
//! with it) till it gathers again, and the liquid streams past as the eye
//! flies. Space stops it.

use std::f32::consts::PI;
use std::time::Duration;

use flow_sdk::fx::{Kind, Sound};
use flow_sdk::input::KeyCode;
use flow_sdk::scene::{add, apply, lerp, lit_as, mul, normalize, rgb, rotation, smooth, sub, Color, Scene, V3};
use flow_sdk::stage::Fx;

use crate::{Demo, Frame, Line, Show};

const GRID: u32 = 0x20ff70;
const HILLS: u32 = 0x50ffb0;
const MOON: u32 = 0xfff0a0;
const STARS: u32 = 0xc8d8ff;
const WORD: u32 = 0x40e0ff;
const LAVA: u32 = 0xff8020;
const EMBER: u32 = 0xff2a10;
const WORDS: u32 = 0x9aa0b4;

/// Thirty frames a second: each is a few hundred lines.
const INTERVAL: Duration = Duration::from_micros(33_333);
/// The grid's squares, and how fast the eye flies over them, a second.
const SQUARE: f32 = 2.0;
const SPEED: f32 = 4.0;
/// How high the eye is, and the nearest and farthest it sees the grid.
const EYE: f32 = 1.4;
const NEAR: f32 = 0.8;
const FAR: f32 = 40.0;
/// Radians it sees up and down, and how far above the middle the horizon is,
/// of the height.
const FOV: f32 = 0.9;
const HORIZON: f32 = 0.1;
/// How bright a line is, near to far: its color times these.
const LEVELS: [f32; 3] = [1.0, 0.62, 0.36];
/// The volcano's bearing, right of straight ahead, and how high its top is.
const VOLCANO: f32 = 0.16;
const PEAK: f32 = 0.105;
/// Seconds to the first eruption, between them, and a spark's life.
const FIRST: f32 = 2.5;
const ERUPT: f32 = 7.0;
const SPARK_LIFE: f32 = 2.4;
const SPARKS: u32 = 26;
/// Seconds between the stirs that stream the liquid past.
const STREAM: f32 = 2.5;
/// The word bursting: seconds going apart, held apart and coming back.
const APART: (f32, f32, f32) = (1.1, 0.8, 1.3);
/// Where the word hovers, and how big its letters are.
const WORD_AT: V3 = [0.0, 2.2, -10.0];
const LETTER: f32 = 0.6;
/// Toward Flow's light (`scene::shade`'s): a face turned to it shows its
/// color brightest, so lines can show at full strength.
const TO_LIGHT: V3 = [-0.45, 0.65, 0.61];

/// A small hash, for the hills, the stars and the sparks: the same each run.
fn hash(i: u32, salt: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// How the eye sees the world from where it is: it looks down -z from
/// [`EYE`] above the ground, turned `yaw` right and rolled `roll`.
struct View {
    focal: f32,
    horizon: f32,
    yaw: f32,
    roll: f32,
}

impl View {
    fn new(h: f32, yaw: f32, roll: f32) -> Self {
        Self { focal: h * 0.5 / (FOV * 0.5).tan(), horizon: h * HORIZON, yaw, roll }
    }

    /// A way into the world (x right, y up, z ahead) in the eye's terms.
    fn turn(&self, [x, y, z]: V3) -> V3 {
        let (s, c) = self.yaw.sin_cos();
        [x * c - z * s, y, x * s + z * c]
    }

    /// A point of the world (looking down -z) in the eye's terms.
    fn eye(&self, p: V3) -> V3 {
        self.turn([p[0], p[1] - EYE, -p[2]])
    }

    /// Where a point in the eye's terms shows on the picture (pixels from
    /// its middle, y up), if it's ahead.
    fn show(&self, e: V3) -> Option<[f32; 2]> {
        if e[2] < 0.05 {
            return None;
        }
        let (x, y) = (self.focal * e[0] / e[2], self.focal * e[1] / e[2] + self.horizon);
        let (s, c) = self.roll.sin_cos();
        Some([x * c - y * s, x * s + y * c])
    }

    /// A line of the world as it shows, cut where it passes [`NEAR`]; none
    /// if it's all behind.
    fn line(&self, a: V3, b: V3) -> Option<([f32; 2], [f32; 2])> {
        let (mut a, mut b) = (self.eye(a), self.eye(b));
        if a[2] < NEAR && b[2] < NEAR {
            return None;
        }
        if a[2] < NEAR {
            a = lerp(a, b, (NEAR - a[2]) / (b[2] - a[2]));
        } else if b[2] < NEAR {
            b = lerp(b, a, (NEAR - b[2]) / (a[2] - b[2]));
        }
        Some((self.show(a)?, self.show(b)?))
    }

    /// Where something as far as the horizon shows: `bearing` radians right
    /// of straight ahead and `rise` above the horizon.
    fn sky(&self, bearing: f32, rise: f32) -> Option<[f32; 2]> {
        self.show(self.turn([bearing.sin() * rise.cos(), rise.sin(), bearing.cos() * rise.cos()]))
    }
}

/// A line on the tube: from `a` to `b` in the picture's pixels, its color
/// and how bright (an index into [`LEVELS`]), and `z` above what's under it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Beam {
    a: [f32; 2],
    b: [f32; 2],
    color: u32,
    level: usize,
    z: f32,
}

/// The beams as quads: for each, a wide faint halo (the brightest only),
/// a nearer one, and a core `core` pixels wide, hot toward white. Where
/// lines meet, their halos overlap brighter, as a beam lingering at a
/// corner did.
fn draw_beams(scene: &mut Scene, beams: &[Beam], core: f32) {
    let n = normalize(TO_LIGHT);
    for (layer, (width, alpha, hot)) in [(core * 8.0, 0.09, 0.0), (core * 3.0, 0.28, 0.0), (core, 1.0, 0.45)].into_iter().enumerate() {
        for beam in beams {
            if layer == 0 && beam.level > 0 {
                continue;
            }
            let k = LEVELS[beam.level.min(LEVELS.len() - 1)];
            let c = rgb(beam.color);
            let tone = |v: f32| (v + (1.0 - v) * hot) * k;
            let color: Color = [tone(c[0]), tone(c[1]), tone(c[2]), alpha];
            let (dx, dy) = (beam.b[0] - beam.a[0], beam.b[1] - beam.a[1]);
            let len = (dx * dx + dy * dy).sqrt();
            let (ux, uy) = if len < 1e-3 { (1.0, 0.0) } else { (dx / len, dy / len) };
            let half = width * 0.5;
            let (ax, ay, bx, by) = (beam.a[0] - ux * half, beam.a[1] - uy * half, beam.b[0] + ux * half, beam.b[1] + uy * half);
            let (px, py) = (-uy * half, ux * half);
            let z = beam.z + layer as f32 * 0.3;
            scene.flat(lit_as(color, n), &[[ax - px, ay - py, z], [bx - px, by - py, z], [bx + px, by + py, z], [ax + px, ay + py, z]], n);
        }
    }
}

/// The part of the line from `a` to `b` within `half` of the middle each
/// way (Liang and Barsky's way), if any of it is.
fn clip(a: [f32; 2], b: [f32; 2], half: [f32; 2]) -> Option<([f32; 2], [f32; 2])> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for k in 0..2 {
        for (p, q) in [(-d[k], a[k] + half[k]), (d[k], half[k] - a[k])] {
            if p == 0.0 {
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
    (t0 <= t1).then(|| ([a[0] + d[0] * t0, a[1] + d[1] * t0], [a[0] + d[0] * t1, a[1] + d[1] * t1]))
}

/// The tube's face: a dark rounded screen over the glass, its edge soft,
/// from this far in from the picture's edge to this far, of its height.
const TUBE: (f32, f32) = (0.015, 0.045);

fn draw_tube(scene: &mut Scene, w: f32, h: f32) {
    let n = normalize(TO_LIGHT);
    for (i, alpha) in [0.14, 0.14, 0.14, 0.2].into_iter().enumerate() {
        let inset = h * (TUBE.0 + (TUBE.1 - TUBE.0) * i as f32 / 4.0);
        let (x, y, r) = (w * 0.5 - inset, h * 0.5 - inset, h * 0.09);
        // Each corner a quarter circle in six steps, round from the top right.
        let corners = [(x - r, y - r, 0.0), (-x + r, y - r, 90.0), (-x + r, -y + r, 180.0), (x - r, -y + r, 270.0)];
        let outline: Vec<V3> = corners
            .iter()
            .flat_map(|&(cx, cy, from)| (0..=6).map(move |k| (from + 15.0 * k as f32) * PI / 180.0).map(move |a| [cx + r * a.cos(), cy + r * a.sin(), 0.0]))
            .collect();
        scene.flat([0.0, 0.0, 0.0, alpha], &outline, n);
    }
}

/// How high the hills are at `bearing`: a ridge of waves, with the volcano.
fn hill(bearing: f32) -> f32 {
    let ridge = 0.04 + 0.016 * (bearing * 7.0 + 1.0).sin() + 0.011 * (bearing * 13.0 + 2.0).sin() + 0.006 * (bearing * 31.0).sin();
    let off = (bearing - VOLCANO).abs();
    // A cone with a crater at its top.
    let cone = PEAK - off * 0.85 - (0.014 - off).max(0.0) * 1.2;
    ridge.max(cone)
}

/// The bearings the ridge is drawn through: a step apart, and the crater's
/// edges and floor.
fn ridge() -> Vec<f32> {
    let mut at: Vec<f32> = (0..=48).map(|i| -1.2 + i as f32 * 0.05).chain([VOLCANO - 0.014, VOLCANO, VOLCANO + 0.014]).collect();
    at.sort_by(f32::total_cmp);
    at
}

/// The word's strokes in letter units (each letter 1.2 across and 2 high),
/// laid side by side.
fn word_strokes() -> Vec<([f32; 2], [f32; 2])> {
    let o: Vec<[f32; 2]> = vec![[0.35, 0.0], [0.85, 0.0], [1.2, 0.4], [1.2, 1.6], [0.85, 2.0], [0.35, 2.0], [0.0, 1.6], [0.0, 0.4]];
    let letters: [Vec<([f32; 2], [f32; 2])>; 4] = [
        vec![([0.0, 0.0], [0.0, 2.0]), ([0.0, 2.0], [1.2, 2.0]), ([0.0, 1.1], [0.9, 1.1])],
        vec![([0.0, 2.0], [0.0, 0.0]), ([0.0, 0.0], [1.2, 0.0])],
        (0..o.len()).map(|i| (o[i], o[(i + 1) % o.len()])).collect(),
        vec![([0.0, 2.0], [0.3, 0.0]), ([0.3, 0.0], [0.6, 1.2]), ([0.6, 1.2], [0.9, 0.0]), ([0.9, 0.0], [1.2, 2.0])],
    ];
    letters.into_iter().enumerate().flat_map(|(i, strokes)| strokes.into_iter().map(move |(a, b)| ([a[0] + i as f32 * 1.6, a[1]], [b[0] + i as f32 * 1.6, b[1]]))).collect()
}

/// The word as edges in 3D about its middle: each stroke front and back,
/// and a strut between them at each corner.
fn word_edges() -> Vec<(V3, V3)> {
    let strokes = word_strokes();
    let (mid_x, mid_y, depth) = (3.0, 1.0, 0.18 / LETTER);
    let at = |p: [f32; 2], z: f32| mul([p[0] - mid_x, p[1] - mid_y, z], LETTER);
    let mut edges: Vec<(V3, V3)> = Vec::new();
    let mut corners: Vec<[f32; 2]> = Vec::new();
    for &(a, b) in &strokes {
        edges.push((at(a, depth), at(b, depth)));
        edges.push((at(a, -depth), at(b, -depth)));
        for p in [a, b] {
            if !corners.iter().any(|c| (c[0] - p[0]).abs() < 1e-4 && (c[1] - p[1]).abs() < 1e-4) {
                corners.push(p);
            }
        }
    }
    edges.extend(corners.into_iter().map(|p| (at(p, depth), at(p, -depth))));
    edges
}

/// How far apart the word is `since` it burst: 0 whole, 1 furthest apart.
fn apart(since: f32) -> f32 {
    let (out, held, back) = APART;
    if since < out {
        let x = since / out;
        1.0 - (1.0 - x) * (1.0 - x)
    } else if since < out + held {
        1.0
    } else {
        1.0 - smooth(out + held, out + held + back, since)
    }
}

/// Edge `i` of the word flung `k` of the way apart: out from the middle
/// and turning about its own.
fn flung((a, b): (V3, V3), i: u32, k: f32) -> (V3, V3) {
    if k <= 0.0 {
        return (a, b);
    }
    let mid = mul(add(a, b), 0.5);
    let jitter = [hash(i, 1) - 0.5, hash(i, 2) - 0.5, hash(i, 3) - 0.5];
    let away = mul(normalize(add(mid, mul(jitter, 0.8))), (2.0 + 1.6 * hash(i, 4)) * k);
    let spin = rotation(normalize([hash(i, 5) - 0.5, hash(i, 6) - 0.5, hash(i, 7) - 0.5]), (hash(i, 8) * 2.0 - 1.0) * 6.0 * k);
    let place = |p: V3| add(add(mid, apply(spin, sub(p, mid))), away);
    (place(a), place(b))
}

/// A spark of eruption `n`, `age` seconds out: its bearing and rise.
fn spark(n: u32, j: u32, age: f32) -> (f32, f32) {
    let k = n * 97 + j;
    let (across, up) = ((hash(k, 11) - 0.5) * 0.16, 0.09 + 0.09 * hash(k, 12));
    (VOLCANO + across * age, PEAK + up * age - 0.06 * age * age)
}

pub struct Vector {
    t: f32,
    /// How far the eye has flown.
    flown: f32,
    stopped: bool,
    /// When the word burst, while it's apart.
    burst: Option<f32>,
    clicked: bool,
    streamed: f32,
}

impl Vector {
    pub fn new() -> Self {
        Self { t: 0.0, flown: 0.0, stopped: false, burst: None, clicked: false, streamed: 0.0 }
    }

    fn view(&self, h: f32) -> View {
        View::new(h, 0.1 * (self.t * 0.21).sin(), 0.035 * (self.t * 0.21 + 0.8).sin())
    }

    /// Where the word is, turned and bobbing, and how far apart.
    fn word(&self) -> (V3, f32, f32) {
        let at = add(WORD_AT, [0.0, 0.15 * (self.t * 0.8).sin(), 0.0]);
        (at, self.t * 0.55, self.burst.map_or(0.0, |b| apart(self.t - b)))
    }

    /// Every line in the picture, `w` × `h` pixels.
    fn beams(&self, view: &View, w: f32, h: f32) -> Vec<Beam> {
        let mut beams = Vec::new();
        let mut push = |line: Option<([f32; 2], [f32; 2])>, color: u32, level: usize, z: f32| {
            if let Some((a, b)) = line {
                beams.push(Beam { a, b, color, level, z });
            }
        };
        let level = |d: f32| if d < 14.0 { 0 } else if d < 26.0 { 1 } else { 2 };
        // The grid: lines running ahead, dimmer where they're far, and
        // lines across coming toward the eye.
        for i in -10..=10 {
            let x = i as f32 * SQUARE;
            for (near, far) in [(0.0, 14.0), (14.0, FAR)] {
                push(view.line([x, 0.0, -near], [x, 0.0, -far]), GRID, level(near), 1.0);
            }
        }
        let phase = self.flown.rem_euclid(SQUARE);
        for k in 1..=(FAR / SQUARE) as i32 {
            let d = k as f32 * SQUARE - phase;
            push(view.line([-20.0, 0.0, -d], [20.0, 0.0, -d]), GRID, level(d), 1.0);
        }
        // The hills on the horizon, and the volcano among them.
        let ridge = ridge();
        for pair in ridge.windows(2) {
            push(view.sky(pair[0], hill(pair[0])).zip(view.sky(pair[1], hill(pair[1]))), HILLS, 0, 1.0);
        }
        // The moon, a crescent: the outer edge's arc, and the inner's back.
        if let Some(c) = view.sky(-0.42, 0.25) {
            let r = h * 0.055;
            let mut edge: Vec<[f32; 2]> = (0..=9).map(|i| (70.0 + 220.0 * i as f32 / 9.0) * PI / 180.0).map(|a| [c[0] + r * a.cos(), c[1] + r * a.sin()]).collect();
            edge.extend((1..=6).map(|i| (252.7 - 145.4 * i as f32 / 6.0) * PI / 180.0).map(|a| [c[0] + r * (0.634 + 0.984 * a.cos()), c[1] + r * 0.984 * a.sin()]));
            for pair in edge.windows(2) {
                push(Some((pair[0], pair[1])), MOON, 0, 1.0);
            }
        }
        // Stars, twinkling.
        for s in 0..18 {
            let (bearing, rise) = ((hash(s, 21) - 0.5) * 2.4, 0.09 + 0.36 * hash(s, 22));
            if let Some(p) = view.sky(bearing, rise) {
                let lit = (self.t * (1.0 + 2.0 * hash(s, 23)) + hash(s, 24) * 7.0).sin() > -0.3;
                push(Some((p, [p[0] + 1.5, p[1]])), STARS, if lit { 0 } else { 2 }, 1.0);
            }
        }
        // The sparks of the eruption under way.
        if self.t >= FIRST {
            let n = ((self.t - FIRST) / ERUPT) as u32;
            let age = self.t - FIRST - n as f32 * ERUPT;
            if age < SPARK_LIFE {
                for j in 0..SPARKS {
                    let (b0, r0) = spark(n, j, (age - 0.08).max(0.0));
                    let (b1, r1) = spark(n, j, age);
                    if r1 > hill(b1) {
                        let color = if age < 1.1 { LAVA } else { EMBER };
                        push(view.sky(b0, r0).zip(view.sky(b1, r1)), color, if age < 1.8 { 0 } else { 1 }, 2.0);
                    }
                }
            }
        }
        // The word, turning, whole or flung apart.
        let (at, turn, k) = self.word();
        let spin = rotation([0.0, 1.0, 0.0], turn);
        for (i, edge) in word_edges().into_iter().enumerate() {
            let (a, b) = flung(edge, i as u32, k);
            push(view.line(add(at, apply(spin, a)), add(at, apply(spin, b))), WORD, 0, 3.0);
        }
        // The beam draws on the tube's face and nowhere past it.
        let edge = h * TUBE.1;
        beams
            .into_iter()
            .filter_map(|b| {
                let (a, c) = clip(b.a, b.b, [w * 0.5 - edge, h * 0.5 - edge])?;
                Some(Beam { a, b: c, ..b })
            })
            .collect()
    }
}

impl Default for Vector {
    fn default() -> Self {
        Self::new()
    }
}

impl Demo for Vector {
    fn interval(&self) -> Duration {
        INTERVAL
    }

    fn words(&self) -> u16 {
        2
    }

    fn frame(&mut self, dt: f32, w: f32, h: f32) -> Frame {
        let before = self.t;
        let mut effects = Vec::new();
        if !self.stopped {
            self.t += dt;
            self.flown += dt * SPEED;
        }
        let view = self.view(h);
        let (at, _, _) = self.word();
        let word_at = view.show(view.eye(at));
        if std::mem::take(&mut self.clicked) && self.burst.is_none() {
            self.burst = Some(self.t);
            effects.extend(word_at.map(|p| Fx::new(Kind::Burst, p, w * 0.12, 0.7).color(WORD).sound(Sound::Thunder)));
        }
        if let Some(b) = self.burst {
            if self.t - b >= APART.0 + APART.1 + APART.2 {
                self.burst = None;
                effects.extend(word_at.map(|p| Fx::new(Kind::Ripple, p, w * 0.15, 0.45).sound(Sound::Chime)));
            }
        }
        // An eruption strikes the liquid with lightning from the crater up.
        let eruptions = |t: f32| if t < FIRST { 0 } else { ((t - FIRST) / ERUPT) as u32 + 1 };
        if eruptions(self.t) > eruptions(before) {
            if let (Some(top), Some(sky)) = (view.sky(VOLCANO, PEAK), view.sky(VOLCANO + 0.03, PEAK + 0.16)) {
                effects.push(Fx::new(Kind::Bolt, top, w * 0.01, 0.6).to(sky).color(LAVA));
                effects.push(Fx::new(Kind::Burst, top, w * 0.06, 0.5).color(LAVA).quiet());
            }
        }
        if !self.stopped && self.t - self.streamed >= STREAM {
            self.streamed = self.t;
            effects.push(Fx::new(Kind::Stir, [0.0, -h * 0.3], w * 0.4, 0.3).dir([0.0, -1.0]).quiet());
        }

        let mut scene = Scene::new(w, h);
        draw_tube(&mut scene, w, h);
        draw_beams(&mut scene, &self.beams(&view, w, h), (h * 0.0028).max(1.4));
        let words = vec![
            Line::new("Vector: lines as the arcades' vector monitors drew them, around 1980", WORDS),
            Line::new(if self.stopped { "Stopped · space flies on · q quits" } else { "Best under Flow's CRT (Settings › CRT) · click to burst the word · q quits" }, WORDS),
        ];
        Frame { show: Show::Scene(scene), effects, words }
    }

    fn key(&mut self, key: KeyCode) {
        if key == KeyCode::Char(' ') {
            self.stopped = !self.stopped;
        }
    }

    fn click(&mut self, _at: [f32; 2]) {
        self.clicked = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn far_things_show_nearer_the_horizon_and_lines_behind_the_eye_are_cut() {
        let view = View::new(800.0, 0.0, 0.0);
        let near = view.eye([0.0, 0.0, -2.0]);
        let far = view.eye([0.0, 0.0, -50.0]);
        let (near, far) = (view.show(near).unwrap(), view.show(far).unwrap());
        assert!(near[1] < far[1] && far[1] < view.horizon, "the ground runs up to the horizon");
        assert!((view.sky(0.0, 0.0).unwrap()[1] - view.horizon).abs() < 1e-3);
        assert!(view.sky(0.3, 0.0).unwrap()[0] > 0.0, "a bearing to the right shows right");
        // A line from behind the eye to ahead of it starts at the near plane.
        let (a, _) = view.line([1.0, 0.0, 5.0], [1.0, 0.0, -5.0]).expect("partly ahead");
        let cut = view.show(view.eye([1.0, 0.0, -NEAR])).unwrap();
        assert!((a[0] - cut[0]).abs() < 1e-2 && (a[1] - cut[1]).abs() < 1e-2);
        assert_eq!(view.line([0.0, 0.0, 5.0], [1.0, 0.0, 1.0]), None, "all behind");
    }

    #[test]
    fn lines_are_cut_to_the_tubes_face() {
        let half = [100.0, 50.0];
        assert_eq!(clip([-300.0, 0.0], [300.0, 0.0], half), Some(([-100.0, 0.0], [100.0, 0.0])));
        assert_eq!(clip([0.0, 0.0], [10.0, 10.0], half), Some(([0.0, 0.0], [10.0, 10.0])), "inside, as it was");
        assert_eq!(clip([0.0, 60.0], [10.0, 80.0], half), None);
        let (a, b) = clip([0.0, 0.0], [200.0, 200.0], half).expect("partly inside");
        assert_eq!((a, b), ([0.0, 0.0], [50.0, 50.0]));
        let mut v = Vector::new();
        v.frame(0.5, 1200.0, 700.0);
        let edge = 700.0 * TUBE.1;
        assert!(v.beams(&v.view(700.0), 1200.0, 700.0).iter().all(|b| [b.a, b.b].iter().all(|p| p[0].abs() <= 600.0 - edge + 0.01 && p[1].abs() <= 350.0 - edge + 0.01)));
    }

    #[test]
    fn a_beam_is_a_hot_core_inside_two_halos() {
        let mut scene = Scene::new(400.0, 300.0);
        draw_beams(&mut scene, &[Beam { a: [-50.0, 0.0], b: [50.0, 0.0], color: GRID, level: 0, z: 1.0 }], 2.0);
        assert_eq!(scene.triangles(), 6);
        let json = glb_json(&scene.glb((1.0, 1.0)));
        let alphas: Vec<&str> = json.split("\"baseColorFactor\":[").skip(1).map(|m| m.split(']').next().unwrap().rsplit(',').next().unwrap()).collect();
        assert_eq!(alphas, ["0.09", "0.28", "1", "0"], "the halos see-through, the core solid, then the picture's corners");
        assert_eq!((json.matches("\"alphaMode\":\"OPAQUE\"").count(), json.matches("\"alphaMode\":\"BLEND\"").count()), (1, 3));
    }

    #[test]
    fn the_hills_rise_to_the_volcano_with_a_crater_at_its_top() {
        assert!(hill(VOLCANO + 0.014) > hill(VOLCANO), "its rim stands over its crater");
        assert!(hill(VOLCANO + 0.014) > hill(VOLCANO + 0.2) + 0.03);
        assert!((0..240).map(|i| -1.2 + i as f32 * 0.01).all(|b| (0.0..0.12).contains(&hill(b))));
        assert!(ridge().windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn the_word_comes_apart_on_a_click_and_gathers_again() {
        assert_eq!(apart(0.0), 0.0);
        assert_eq!(apart(APART.0 + 0.1), 1.0);
        assert_eq!(apart(APART.0 + APART.1 + APART.2), 0.0);
        let edge = word_edges()[3];
        assert_eq!(flung(edge, 3, 0.0), edge, "whole, every edge where it was");
        let (a, b) = flung(edge, 3, 1.0);
        let len = |(a, b): (V3, V3)| sub(a, b).iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((len((a, b)) - len(edge)).abs() < 1e-3, "flung, it keeps its length");

        let mut v = Vector::new();
        v.frame(0.1, 1200.0, 700.0);
        v.click([0.0, 0.0]);
        let burst = v.frame(0.1, 1200.0, 700.0);
        assert!(burst.effects.iter().any(|f| f.kind == Kind::Burst && f.color == Some(WORD)));
        let mut gathered = Vec::new();
        for _ in 0..40 {
            gathered.extend(v.frame(0.1, 1200.0, 700.0).effects);
        }
        assert!(gathered.iter().any(|f| f.kind == Kind::Ripple), "it rings as it's whole again");
        assert_eq!(v.burst, None);
    }

    #[test]
    fn the_volcano_strikes_the_liquid_as_it_erupts_and_the_liquid_streams_past() {
        let mut v = Vector::new();
        let mut kinds = Vec::new();
        for _ in 0..(20.0 * 30.0) as usize {
            kinds.extend(v.frame(1.0 / 30.0, 1200.0, 700.0).effects.into_iter().map(|f| f.kind));
        }
        // At 2.5, 9.5 and 16.5 seconds.
        assert_eq!(kinds.iter().filter(|k| **k == Kind::Bolt).count(), 3);
        let stirs = kinds.iter().filter(|k| **k == Kind::Stir).count();
        assert!((7..=8).contains(&stirs), "{stirs} in 20 seconds");
    }

    #[test]
    fn a_frame_is_lines_on_a_tube_inside_the_picture() {
        let mut v = Vector { t: FIRST + 0.5, ..Vector::new() };
        let frame = v.frame(0.0, 1200.0, 700.0);
        let scene = frame.scene().expect("a scene");
        assert!((600..6000).contains(&scene.triangles()), "{}", scene.triangles());
        let view = v.view(700.0);
        let beams = v.beams(&view, 1200.0, 700.0);
        assert!(beams.iter().any(|b| b.color == LAVA), "the eruption's sparks");
        assert!(beams.iter().any(|b| b.color == WORD) && beams.iter().any(|b| b.color == MOON));
        let glb = scene.glb((1.0, 1.0));
        assert!(glb.starts_with(b"glTF") && glb_json(&glb).contains("\"generator\":\"flow-sdk\""));
        assert!(frame.words[1].text.contains("CRT"), "it says what it's best under");
    }

    /// A GLB's JSON chunk.
    fn glb_json(glb: &[u8]) -> String {
        let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        assert_eq!(&glb[16..20], b"JSON");
        String::from_utf8(glb[20..20 + len].to_vec()).unwrap()
    }
}
