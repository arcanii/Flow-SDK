//! Tint: a day passing over hills, from dawn to night and round again in a
//! little over a minute. The hills, a village, the clouds, the sun, the
//! moon and the stars are a 3D scene sent with the Ratty graphics protocol,
//! as the other scenes are, and the sky is left empty: it's Flow's glass,
//! which the demo washes in the sky's color with OSC 7731's `fx=tint`
//! while it holds the alternate screen (`flow_sdk::fx::tint`). The wash is
//! worked out for each quarter of an hour of the day and sent only when it
//! changes, and Flow eases the glass toward each one; leaving, the demo
//! washes it off (`fx::UNTINT`), as leaving the alternate screen would.
//!
//! Sunrise bursts the liquid gold with a chime, sunset red with two soft
//! falling notes, a breeze stirs it by day, a shooting star at night pushes
//! it along its way, and a click ripples it in the sky's color. ← and → go
//! an hour back or on; space stops the day.

use std::f32::consts::{PI, TAU};
use std::time::Duration;

use flow_sdk::fx::{self, Kind, Sound};
use flow_sdk::input::KeyCode;
use flow_sdk::scene::{lit_as, normalize, rgb, smooth, Scene, V3};
use flow_sdk::stage::Fx;

use crate::cells::blend;
use crate::{Demo, Frame, Line, Show, Told};

/// Seconds a whole day takes, and the hour it begins at.
const DAY: f32 = 72.0;
const START: f32 = 5.0;
/// The wash is worked out for each of these parts of an hour.
const STEPS: f32 = 4.0;
/// Twenty frames a second: the sun and the clouds move slowly, and each
/// frame is a scene of some 60 KB.
const INTERVAL: Duration = Duration::from_millis(50);
/// Seconds between a night's shooting stars, how long one shows, and
/// between the day's breezes.
const SHOOT: f32 = 5.0;
const STREAK: f32 = 0.7;
const BREEZE: f32 = 8.0;
/// The sun and moon go down behind the far hills at this height, of the
/// picture's from its middle.
const BASE: f32 = -0.08;
/// Toward Flow's light (`scene::shade`'s): every face is turned to it, so
/// that bright colors (the sun, the moon, a lit window) show at full
/// strength, as the vector display's lines do. Faced straight out, a face
/// shows at most four fifths of its color's brightness.
const TO_LIGHT: V3 = [-0.45, 0.65, 0.61];

/// The sky through the day: an hour, the color the glass is washed in and
/// how far, deep blue at night, rose and gold at dawn and dusk.
const WASH: [(f32, u32, f32); 14] = [
    (0.0, 0x0a1430, 0.65),
    (4.75, 0x0a1430, 0.65),
    (5.5, 0x2c2a60, 0.55),
    (6.0, 0xc0587a, 0.5),
    (6.5, 0xff9a5a, 0.45),
    (7.5, 0x9ac0e8, 0.4),
    (9.0, 0x4a8ed8, 0.45),
    (15.5, 0x4a8ed8, 0.45),
    (17.0, 0xe8b468, 0.42),
    (18.0, 0xff6a3a, 0.5),
    (18.6, 0x9a3a78, 0.55),
    (19.4, 0x2c2a60, 0.6),
    (20.25, 0x0a1430, 0.65),
    (24.0, 0x0a1430, 0.65),
];

const WORDS: u32 = 0x9aa0b4;
const SUN_HIGH: u32 = 0xfff0a0;
const SUN_LOW: u32 = 0xff6a2a;
const MOON: u32 = 0xeceade;
const MARIA: u32 = 0xcac7ba;
const STAR: u32 = 0xdfe8ff;
const LIT: u32 = 0xffd166;

/// The sky's wash at `hour`, between the hours [`WASH`] names.
pub fn wash_at(hour: f32) -> (u32, f32) {
    let hour = hour.rem_euclid(24.0);
    let i = WASH.iter().rposition(|&(h, _, _)| h <= hour).unwrap_or(0).min(WASH.len() - 2);
    let ((h0, c0, s0), (h1, c1, s1)) = (WASH[i], WASH[i + 1]);
    let k = ((hour - h0) / (h1 - h0).max(1e-3)).clamp(0.0, 1.0);
    (blend(c0, c1, k), s0 + (s1 - s0) * k)
}

/// The wash for the quarter of an hour `hour` is in, as it goes out.
pub fn wash_escape(hour: f32) -> String {
    let (color, strength) = wash_at((hour * STEPS).floor() / STEPS);
    fx::tint(color, strength)
}

/// How high the sun is: 1 at noon, 0 as it rises at six and sets at
/// eighteen, below 0 at night.
fn sun_height(hour: f32) -> f32 {
    (PI * (hour - 6.0) / 12.0).sin()
}

/// How much of the day's light there is, 0 at night to 1.
fn daylight(hour: f32) -> f32 {
    smooth(-0.15, 0.3, sun_height(hour))
}

/// How warm the light is, near sunrise and sunset.
fn warmth(hour: f32) -> f32 {
    let s = sun_height(hour);
    smooth(-0.25, 0.0, s) * (1.0 - smooth(0.1, 0.4, s))
}

/// The part of the day `hour` is in.
pub fn phase(hour: f32) -> &'static str {
    match hour.rem_euclid(24.0) {
        h if h < 5.0 => "night",
        h if h < 6.0 => "first light",
        h if h < 7.0 => "sunrise",
        h if h < 11.0 => "morning",
        h if h < 13.0 => "midday",
        h if h < 17.0 => "afternoon",
        h if h < 18.0 => "evening",
        h if h < 18.75 => "sunset",
        h if h < 20.0 => "dusk",
        _ => "night",
    }
}

/// A small hash: the same each run.
fn hash(i: u32, salt: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// The hills, far to near: their ridge's height at `u` (across, in the
/// picture's heights from its middle), of the picture's height from its
/// middle, and their colors by day, at night and as the sun is low.
struct Hills {
    ridge: fn(f32) -> f32,
    day: u32,
    night: u32,
    warm: u32,
    z: f32,
}

const HILLS: [Hills; 3] = [
    Hills { ridge: far_ridge, day: 0x86a6c4, night: 0x1c2444, warm: 0xd08a8a, z: 3.0 },
    Hills { ridge: middle_ridge, day: 0x4f8f62, night: 0x151d34, warm: 0x8a5068, z: 4.0 },
    Hills { ridge: near_ridge, day: 0x2f6b3c, night: 0x0d1426, warm: 0x5a3048, z: 5.0 },
];

fn far_ridge(u: f32) -> f32 {
    -0.03 + 0.05 * (u * 2.1 + 0.3).sin() + 0.025 * (u * 5.3 + 1.1).sin() + 0.012 * (u * 11.0).sin()
}

fn middle_ridge(u: f32) -> f32 {
    -0.15 + 0.05 * (u * 1.7 + 2.0).sin() + 0.02 * (u * 4.1 + 0.5).sin()
}

fn near_ridge(u: f32) -> f32 {
    -0.28 + 0.045 * (u * 1.3 + 4.0).sin() + 0.018 * (u * 3.7 + 1.7).sin()
}

/// Where the village's houses stand on the near hills, across the
/// picture's width from its middle.
const HOUSES: [f32; 5] = [-0.34, -0.29, -0.235, 0.2, 0.265];
/// And pines, on the near hills and the middle ones.
const PINES: [(usize, f32); 9] = [(2, -0.42), (2, -0.16), (2, -0.12), (2, 0.08), (2, 0.36), (1, -0.05), (1, 0.0), (1, 0.31), (1, 0.4)];

/// Whether house `i`'s window is lit at `hour`: as the light goes, each
/// in its own time, until late at night.
pub fn window_lit(i: usize, hour: f32) -> bool {
    let h = hour.rem_euclid(24.0);
    daylight(hour) < 0.35 + 0.25 * hash(i as u32, 31) && !(1.0..5.5).contains(&h)
}

/// How bright star `i` is at `hour`, `t` seconds in: none by day.
pub fn star_alpha(i: u32, hour: f32, t: f32) -> f32 {
    let night = 1.0 - daylight(hour);
    let twinkle = 0.55 + 0.45 * (t * (1.0 + 2.0 * hash(i, 41)) + hash(i, 42) * TAU).sin();
    (night * night * twinkle).clamp(0.0, 1.0)
}

/// Whether `at` falls after `before` and by `after`, going round midnight.
fn crossed(before: f32, after: f32, at: f32) -> bool {
    if after >= before {
        before < at && at <= after
    } else {
        at > before || at <= after
    }
}

pub struct Tint {
    hour: f32,
    /// Seconds the demo has run, for the clouds and the stars.
    t: f32,
    stopped: bool,
    told: Told,
    /// The wash last worked out, the one waiting to go, and how many have.
    sent: Option<String>,
    waiting: Option<String>,
    sends: u32,
    clicked: Vec<[f32; 2]>,
    /// The shooting star's number and when it began, while it shows.
    star: Option<(u32, f32)>,
    breezed: f32,
}

impl Tint {
    pub fn new() -> Self {
        Self { hour: START, t: 0.0, stopped: false, told: Told { flow: true, effects: true }, sent: None, waiting: None, sends: 0, clicked: Vec::new(), star: None, breezed: 0.0 }
    }

    /// Where the sun is in a picture `w` × `h`, and how big.
    fn sun(&self, w: f32, h: f32) -> ([f32; 2], f32) {
        let a = PI * (self.hour - 6.0) / 12.0;
        ([-a.cos() * 0.44 * w, h * (BASE + a.sin() * (0.38 - BASE))], h * 0.05)
    }

    /// Where the moon is, while it's up (from half past six in the evening
    /// to half past five).
    fn moon(&self, w: f32, h: f32) -> Option<([f32; 2], f32)> {
        let m = (self.hour - 18.5).rem_euclid(24.0);
        (m <= 11.0).then(|| {
            let a = PI * m / 11.0;
            ([-a.cos() * 0.4 * w, h * (BASE + a.sin() * (0.36 - BASE))], h * 0.036)
        })
    }

    /// The shooting star `n`'s head and tail `age` seconds in.
    fn streak(n: u32, age: f32, w: f32, h: f32) -> ([f32; 2], [f32; 2]) {
        let start = [(hash(n, 51) - 0.6) * w * 0.8, h * (0.22 + 0.2 * hash(n, 52))];
        let way = [w * 0.28, -h * 0.12];
        let k = (age / STREAK).clamp(0.0, 1.0);
        let head = [start[0] + way[0] * k, start[1] + way[1] * k];
        let tail = [head[0] - way[0] * 0.25 * k.min(0.5) * 2.0, head[1] - way[1] * 0.25 * k.min(0.5) * 2.0];
        (head, tail)
    }

    fn draw(&self, w: f32, h: f32) -> Scene {
        let mut scene = Scene::new(w, h);
        let n = normalize(TO_LIGHT);
        let flat = |scene: &mut Scene, color: u32, alpha: f32, points: &[[f32; 2]], z: f32| {
            let pts: Vec<[f32; 3]> = points.iter().map(|&[x, y]| [x, y, z]).collect();
            let mut c = rgb(color);
            c[3] = alpha;
            scene.flat(lit_as(c, n), &pts, n);
        };
        let disc = |scene: &mut Scene, color: u32, alpha: f32, at: [f32; 2], r: [f32; 2], z: f32| {
            let pts: Vec<[f32; 2]> = (0..20).map(|i| i as f32 / 20.0 * TAU).map(|a| [at[0] + r[0] * a.cos(), at[1] + r[1] * a.sin()]).collect();
            flat(scene, color, alpha, &pts, z);
        };
        let (light, warm) = (daylight(self.hour), warmth(self.hour));
        let (sky, _) = wash_at(self.hour);

        // Stars, behind everything, coming out as the light goes: their
        // brightness in tenths, so that stars alike share a part of the glTF.
        let far = HILLS[0].ridge;
        for i in 0..48u32 {
            let alpha = (star_alpha(i, self.hour, self.t) * 10.0).round() / 10.0;
            let (x, y) = ((hash(i, 21) - 0.5) * w, h * (BASE + 0.06 + hash(i, 22) * (0.47 - BASE - 0.06)));
            if alpha < 0.03 || y < h * (far(x / h * 3.0) + 0.03) {
                continue;
            }
            let s = (h * 0.0035).max(1.5) * (0.7 + 0.6 * hash(i, 23));
            flat(&mut scene, STAR, alpha, &[[x - s, y - s], [x + s, y - s], [x + s, y + s], [x - s, y + s]], 0.5);
        }
        // The sun, with a glow about it while it's low, and the moon.
        let (sun, r) = self.sun(w, h);
        if sun_height(self.hour) > -0.2 {
            let high = smooth(0.0, 0.45, sun_height(self.hour));
            disc(&mut scene, blend(SUN_LOW, SUN_HIGH, high), 0.16 * (1.0 - high) + 0.06, sun, [r * 2.2, r * 2.2], 0.9);
            disc(&mut scene, blend(SUN_LOW, SUN_HIGH, high), 1.0, sun, [r, r], 1.0);
        }
        if let Some((moon, r)) = self.moon(w, h) {
            disc(&mut scene, MOON, 1.0, moon, [r, r], 1.0);
            for (dx, dy, k) in [(-0.3, 0.25, 0.28), (0.25, -0.1, 0.22), (0.05, 0.42, 0.14), (-0.1, -0.4, 0.18)] {
                disc(&mut scene, MARIA, 1.0, [moon[0] + dx * r, moon[1] + dy * r], [r * k, r * k], 1.1);
            }
        }
        // A shooting star.
        if let Some((n, began)) = self.star {
            let (head, tail) = Self::streak(n, self.t - began, w, h);
            let fade = 1.0 - smooth(STREAK * 0.6, STREAK, self.t - began);
            let s = (h * 0.003).max(1.2);
            flat(&mut scene, 0xffffff, 0.9 * fade, &[[tail[0], tail[1] - s * 0.3], [head[0], head[1] - s], [head[0], head[1] + s], [tail[0], tail[1] + s * 0.3]], 1.2);
        }
        // The clouds, drifting right, pink and gold as the sun is low.
        let cloud = blend(blend(0x30364e, 0xf2f4f8, light), 0xffb090, warm * 0.7);
        for (i, (y, speed, size)) in [(0.36, 0.010, 1.0), (0.27, 0.016, 0.8), (0.42, 0.007, 0.65), (0.2, 0.022, 0.9)].into_iter().enumerate() {
            let span = w + h * 0.5;
            let x = (hash(i as u32, 61) * span + self.t * speed * h).rem_euclid(span) - span * 0.5;
            for (dx, dy, rx, ry) in [(0.0, 0.0, 0.06, 0.03), (0.05, 0.012, 0.045, 0.028), (-0.05, 0.008, 0.04, 0.024), (0.095, -0.004, 0.03, 0.018), (-0.09, -0.004, 0.028, 0.016)] {
                disc(&mut scene, cloud, 1.0, [x + dx * h * size, y * h + dy * h * size], [rx * h * size, ry * h * size], 2.0 + i as f32 * 0.01);
            }
        }
        // The hills, each a strip at a time (a polygon has to be convex),
        // the far ones bluer, as the air between makes them.
        let strips = 64;
        for (k, hills) in HILLS.iter().enumerate() {
            let mut color = blend(blend(hills.night, hills.day, light), hills.warm, warm * 0.45);
            if k == 0 {
                color = blend(color, sky, 0.25);
            }
            for i in 0..strips {
                let (x0, x1) = (-w * 0.5 + w * i as f32 / strips as f32, -w * 0.5 + w * (i + 1) as f32 / strips as f32);
                let (y0, y1) = ((hills.ridge)(x0 / h * 3.0) * h, (hills.ridge)(x1 / h * 3.0) * h);
                flat(&mut scene, color, 1.0, &[[x0, -h * 0.5], [x1, -h * 0.5], [x1, y1], [x0, y0]], hills.z);
            }
        }
        // Pines on the hills, and the village, its windows lit at night.
        let pine = blend(0x0a1220, 0x1f5a2e, light);
        for (layer, at) in PINES {
            let (x, s) = (at * w, h * if layer == 2 { 0.05 } else { 0.035 });
            let ground = (HILLS[layer].ridge)(x / h * 3.0) * h - s * 0.15;
            let z = HILLS[layer].z + 0.5;
            flat(&mut scene, pine, 1.0, &[[x - s * 0.4, ground], [x + s * 0.4, ground], [x, ground + s * 0.8]], z);
            flat(&mut scene, pine, 1.0, &[[x - s * 0.3, ground + s * 0.45], [x + s * 0.3, ground + s * 0.45], [x, ground + s * 1.15]], z + 0.01);
        }
        let (wall, roof) = (blend(blend(0x2c2c3c, 0xe8dcc4, light), 0xffc8a0, warm * 0.3), blend(0x24161e, 0xa8483a, light));
        for (i, at) in HOUSES.iter().enumerate() {
            let (x, s) = (at * w, h * 0.045);
            let ground = (HILLS[2].ridge)(x / h * 3.0) * h - s * 0.2;
            let (half, top) = (s * 0.5, ground + s * 0.7);
            flat(&mut scene, wall, 1.0, &[[x - half, ground], [x + half, ground], [x + half, top], [x - half, top]], 6.0);
            flat(&mut scene, roof, 1.0, &[[x - half * 1.2, top], [x + half * 1.2, top], [x, top + s * 0.5]], 6.0);
            let window = if window_lit(i, self.hour) { LIT } else { blend(0x1a1c28, 0x40506a, light) };
            let (wx, wy, ws) = (x + s * 0.12, ground + s * 0.32, s * 0.14);
            flat(&mut scene, window, 1.0, &[[wx - ws, wy - ws], [wx + ws, wy - ws], [wx + ws, wy + ws], [wx - ws, wy + ws]], 6.1);
        }
        scene
    }

    fn words(&self) -> Vec<Line> {
        let (color, strength) = wash_at((self.hour * STEPS).floor() / STEPS);
        let minutes = (self.hour * 60.0) as u32 % (24 * 60);
        let time = format!("{:02}:{:02} · {}", minutes / 60, minutes % 60, phase(self.hour));
        let first = if self.told.flow { format!("{time} · the glass washed {color:06x} at {strength:.2}") } else { time };
        let second = if self.told.flow {
            let times = if self.sends == 1 { "once".to_string() } else { format!("{} times", self.sends) };
            format!("The sky is Flow's glass, washed with fx=tint as it changes: sent {times}")
        } else {
            "Not in Flow: the wash is Flow's, so this sky stays the terminal's own".to_string()
        };
        let third = if self.stopped { "Stopped · space starts the day again · ← → an hour · q quits" } else { "← → an hour · space stops the day · click the sky · q quits" };
        vec![Line::new(first, blend(color, 0xffffff, 0.6)).bold(), Line::new(second, WORDS), Line::new(third, WORDS)]
    }
}

impl Default for Tint {
    fn default() -> Self {
        Self::new()
    }
}

impl Demo for Tint {
    fn told(&mut self, told: Told) {
        self.told = told;
    }

    fn interval(&self) -> Duration {
        INTERVAL
    }

    fn words(&self) -> u16 {
        3
    }

    fn frame(&mut self, dt: f32, w: f32, h: f32) -> Frame {
        let before = self.hour;
        let mut effects = Vec::new();
        if !self.stopped {
            self.t += dt;
            self.hour = (self.hour + dt * 24.0 / DAY).rem_euclid(24.0);
            // The sun comes up gold with a chime, and goes down red with two
            // soft notes falling.
            let horizon = |east: bool| [(if east { -0.44 } else { 0.44 }) * w, h * BASE];
            if crossed(before, self.hour, 6.0) {
                effects.push(Fx::new(Kind::Burst, horizon(true), w * 0.14, 0.6).color(0xffb040).sound(Sound::Chime));
                effects.push(Fx::new(Kind::Ripple, horizon(true), w * 0.25, 0.4).quiet());
            }
            if crossed(before, self.hour, 18.0) {
                effects.push(Fx::new(Kind::Burst, horizon(false), w * 0.14, 0.6).color(0xff4a2a).sound(Sound::Low));
                effects.push(Fx::new(Kind::Ripple, horizon(false), w * 0.25, 0.4).quiet());
            }
            // By day a breeze, the way the clouds go; at night a shooting
            // star now and then, pushing the liquid along its way.
            if daylight(self.hour) > 0.7 && self.t - self.breezed >= BREEZE {
                self.breezed = self.t;
                effects.push(Fx::new(Kind::Stir, [-w * 0.3, h * 0.25], w * 0.35, 0.3).dir([1.0, 0.0]).quiet());
            }
            let shoot = |t: f32| (t / SHOOT) as u32;
            if daylight(self.hour) < 0.05 && shoot(self.t) > shoot(self.t - dt) {
                let n = shoot(self.t);
                self.star = Some((n, self.t));
                let (head, _) = Self::streak(n, 0.0, w, h);
                effects.push(Fx::new(Kind::Stir, head, w * 0.12, 0.35).dir([0.92, -0.39]).quiet());
            }
        }
        if self.star.is_some_and(|(_, began)| self.t - began > STREAK) {
            self.star = None;
        }
        let (sky, _) = wash_at(self.hour);
        for at in std::mem::take(&mut self.clicked) {
            effects.push(Fx::new(Kind::Ripple, at, w * 0.1, 0.5));
            effects.push(Fx::new(Kind::Burst, at, w * 0.05, 0.4).color(blend(sky, 0xffffff, 0.3)).quiet());
        }
        let wash = wash_escape(self.hour);
        if self.sent.as_ref() != Some(&wash) {
            self.sent = Some(wash.clone());
            self.waiting = Some(wash);
            self.sends += 1;
        }
        Frame { show: Show::Scene(self.draw(w, h)), effects, words: self.words() }
    }

    fn key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Left => self.hour = (self.hour - 1.0).rem_euclid(24.0),
            KeyCode::Right => self.hour = (self.hour + 1.0).rem_euclid(24.0),
            KeyCode::Char(' ') => self.stopped = !self.stopped,
            _ => {}
        }
    }

    fn click(&mut self, at: [f32; 2]) {
        self.clicked.push(at);
    }

    fn wash(&mut self) -> Option<String> {
        self.waiting.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wash_follows_the_sky_through_the_day() {
        assert_eq!(wash_at(2.0), (0x0a1430, 0.65), "deep blue at night");
        assert_eq!(wash_at(12.0), (0x4a8ed8, 0.45), "blue at midday");
        let (dawn, _) = wash_at(6.25);
        assert!(dawn >> 16 > 0xc0, "rose and gold at dawn: {dawn:06x}");
        assert_eq!(wash_at(24.0), wash_at(0.0), "round again");
        assert_eq!(wash_escape(12.1), "\x1b]7731;fx=tint;color=4a8ed8;strength=0.45\x07");
        assert_eq!(wash_escape(6.1), wash_escape(6.24), "a quarter of an hour's the same");
        assert_ne!(wash_escape(6.1), wash_escape(6.26));
    }

    #[test]
    fn the_wash_goes_out_only_when_it_changes() {
        let mut tint = Tint::new();
        let mut sent = Vec::new();
        for _ in 0..(DAY * 30.0) as usize {
            tint.frame(1.0 / 30.0, 1200.0, 700.0);
            sent.extend(tint.wash());
        }
        assert!(sent.windows(2).all(|w| w[0] != w[1]), "never the same twice running");
        assert!((30..=96).contains(&sent.len()), "{} in a day", sent.len());
        assert_eq!(tint.sends as usize, sent.len());
        // From midnight to a quarter to five the night's sky doesn't change.
        let mut night = Tint { hour: 0.1, ..Tint::new() };
        night.frame(0.0, 1200.0, 700.0);
        assert!(night.wash().is_some(), "the first frame washes the glass");
        for _ in 0..(4.0 * DAY / 24.0 * 30.0) as usize {
            night.frame(1.0 / 30.0, 1200.0, 700.0);
            assert_eq!(night.wash(), None);
        }
        // Stopped, nothing changes, so nothing goes.
        let mut stopped = Tint { hour: 6.2, ..Tint::new() };
        stopped.frame(0.0, 800.0, 400.0);
        stopped.wash();
        stopped.key(KeyCode::Char(' '));
        for _ in 0..100 {
            stopped.frame(0.1, 800.0, 400.0);
        }
        assert_eq!((stopped.wash(), stopped.hour), (None, 6.2));
        // An hour on does.
        stopped.key(KeyCode::Right);
        stopped.frame(0.0, 800.0, 400.0);
        assert!(stopped.wash().is_some_and(|w| w.contains("fx=tint")));
    }

    #[test]
    fn sunrise_chimes_and_sunset_falls_as_the_liquid_bursts() {
        let mut tint = Tint { hour: 5.9, ..Tint::new() };
        let rise: Vec<Fx> = (0..30).flat_map(|_| tint.frame(0.1, 1200.0, 700.0).effects).collect();
        let burst = rise.iter().find(|f| f.kind == Kind::Burst).expect("the sun came up");
        assert!(burst.at[0] < 0.0 && burst.sound == flow_sdk::fx::FxSound::Named(Sound::Chime));
        let mut tint = Tint { hour: 17.9, ..Tint::new() };
        let set: Vec<Fx> = (0..30).flat_map(|_| tint.frame(0.1, 1200.0, 700.0).effects).collect();
        let burst = set.iter().find(|f| f.kind == Kind::Burst).expect("the sun went down");
        assert!(burst.at[0] > 0.0 && burst.sound == flow_sdk::fx::FxSound::Named(Sound::Low));
        assert_eq!(set.iter().filter(|f| f.kind == Kind::Burst).count(), 1, "once");
        assert!(crossed(23.9, 0.1, 0.0) && crossed(23.9, 0.1, 24.0) && !crossed(1.0, 2.0, 6.0));
    }

    #[test]
    fn stars_come_out_and_windows_light_as_the_light_goes() {
        assert_eq!(star_alpha(3, 12.0, 1.0), 0.0);
        assert!((0..48).all(|i| star_alpha(i, 0.0, 2.0) > 0.08));
        assert!(HOUSES.iter().enumerate().all(|(i, _)| !window_lit(i, 12.0) && window_lit(i, 21.0)));
        assert!(!window_lit(0, 3.0), "and go out late at night");
        // A shooting star now and then at night, none by day.
        let mut night = Tint { hour: 23.0, ..Tint::new() };
        let stirs = (0..310).flat_map(|_| night.frame(0.05, 1200.0, 700.0).effects).filter(|f| f.kind == Kind::Stir).count();
        assert_eq!(stirs, (15.5 / SHOOT) as usize);
    }

    #[test]
    fn the_hills_leave_the_sky_to_the_glass() {
        for hills in &HILLS {
            assert!((-200..200).map(|i| i as f32 * 0.03).all(|u| (hills.ridge)(u) < 0.06 && (hills.ridge)(u) > -0.4));
        }
        let mean = |k: usize| (0..200).map(|i| (HILLS[k].ridge)(i as f32 * 0.03)).sum::<f32>() / 200.0;
        assert!(mean(0) > mean(1) && mean(1) > mean(2), "far ones higher up the picture");
        let mut tint = Tint { hour: 12.0, ..Tint::new() };
        let frame = tint.frame(0.0, 1200.0, 700.0);
        let triangles = frame.scene().expect("a scene").triangles();
        assert!((300..2500).contains(&triangles), "{triangles}");
        // Noon's sun is high in the middle; the moon's down.
        let (sun, _) = tint.sun(1200.0, 700.0);
        assert!(sun[0].abs() < 1.0 && sun[1] > 200.0);
        assert!(tint.moon(1200.0, 700.0).is_none());
        assert!(Tint { hour: 0.0, ..Tint::new() }.moon(1200.0, 700.0).is_some());
    }

    #[test]
    fn its_words_tell_the_time_and_the_wash_or_that_its_not_flow() {
        let mut tint = Tint { hour: 18.2, ..Tint::new() };
        let frame = tint.frame(0.0, 800.0, 400.0);
        assert!(frame.words[0].text.starts_with("18:12 · sunset · the glass washed "), "{}", frame.words[0].text);
        assert!(frame.words[1].text.contains("fx=tint") && frame.words[1].text.ends_with("sent once"));
        tint.told(Told { flow: false, effects: false });
        let frame = tint.frame(0.0, 800.0, 400.0);
        assert!(frame.words[1].text.starts_with("Not in Flow"));
        assert!(frame.words.iter().all(|l| l.text.chars().count() <= 80), "fits 80 columns");
        tint.key(KeyCode::Left);
        assert!((tint.hour - 17.2).abs() < 1e-4);
        tint.click([10.0, 50.0]);
        assert!(tint.frame(0.0, 800.0, 400.0).effects.iter().any(|f| f.kind == Kind::Ripple && f.at == [10.0, 50.0]));
    }
}
