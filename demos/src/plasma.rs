//! Plasma: the demoscene's sum of waves, its colors cycling through it,
//! made in code each frame as a picture and sent with kitty's graphics
//! protocol under one id, which Flow shows in the last one's place
//! ([`crate::pictures::Easel`]). Its dark bands are see-through, so the
//! liquid shows between them, and it rides the liquid as Flow's pictures
//! do, bending where it flows: the liquid is stirred along with the
//! plasma's eye as it wanders. A click drops a stone, which rings out
//! through the plasma and ripples the liquid; ← and → change its colors;
//! space stops it.

use std::time::Duration;

use flow_sdk::fx::Kind;
use flow_sdk::input::KeyCode;
use flow_sdk::picture::Image;
use flow_sdk::stage::Fx;

use crate::{Demo, Frame, Line, Show, Shows};

const WORDS: u32 = 0x9aa0b4;
/// About how many pixels the picture has: Flow scales it smoothly to its
/// cells, and a plasma is soft anyway, so a small one sent often does.
const PIXELS: f32 = 40_000.0;
/// Thirty frames a second are plenty for waves.
const INTERVAL: Duration = Duration::from_micros(33_333);
/// Seconds between the stirs that follow its eye.
const STIR: f32 = 1.5;
/// How fast a stone's ring goes out, in picture heights a second, and how
/// long it lasts.
const RING_SPEED: f32 = 0.55;
const RING_LIFE: f32 = 4.0;
/// How far in from each edge the picture fades out, of its height.
const FADE: f32 = 0.12;
/// How many times round its palette the plasma's values go, and how many
/// colors the palette has: 256, as a VGA card's plasmas had, which also
/// leaves the picture runs of a color to compress.
const BANDS: f32 = 3.0;
const COLORS: f32 = 256.0;

/// A palette: its name, and colors (RGBA, straight) evenly round its
/// cycle, which the plasma's value runs through.
struct Palette {
    name: &'static str,
    stops: &'static [u32],
}

const PALETTES: [Palette; 4] = [
    // A rainbow, nearly opaque throughout, as the Amiga's plasmas were.
    Palette { name: "rainbow", stops: &[0xff3050e0, 0xffb020e8, 0xf0f040f0, 0x30e070e0, 0x2080ffe0, 0x8040ffe8] },
    // Black and clear at its coolest, white at its hottest.
    Palette { name: "fire", stops: &[0x00000000, 0x80100060, 0xe0301ab0, 0xff8a10e0, 0xffe040ff, 0xfffff0ff, 0xffe040ff, 0xff8a10e0, 0xe0301ab0, 0x80100060] },
    // Deep water clear, its crests white.
    Palette { name: "sea", stops: &[0x04103010, 0x0a3a9070, 0x1890d0c0, 0x50e0f0e8, 0xf0ffffff, 0x50e0f0e8, 0x1890d0c0, 0x0a3a9070] },
    // Violet and rose, after Flow's own glass.
    Palette { name: "glass", stops: &[0x30106020, 0x7040c090, 0xc070f0d0, 0xff90d0f0, 0xffe0f0ff, 0xff90d0f0, 0xc070f0d0, 0x7040c090] },
];

impl Palette {
    /// The color `i` of the way round, 0 to 1, as RGBA bytes.
    fn at(&self, i: f32) -> [u8; 4] {
        let n = self.stops.len();
        let x = i.rem_euclid(1.0) * n as f32;
        let (k, f) = (x as usize % n, x.fract());
        let (a, b) = (self.stops[k], self.stops[(k + 1) % n]);
        let byte = |c: u32, s: u32| ((c >> s) & 0xff) as f32;
        [24, 16, 8, 0].map(|s| (byte(a, s) + (byte(b, s) - byte(a, s)) * f).round() as u8)
    }

    /// Its color `i` round as 0xRRGGBB, for the liquid's bursts.
    fn rgb(&self, i: f32) -> u32 {
        let [r, g, b, _] = self.at(i);
        u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
    }
}

/// A stone dropped in the plasma: where, in picture heights from its
/// middle (y up), and when.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Stone {
    at: [f32; 2],
    born: f32,
}

#[derive(Clone)]
pub struct Plasma {
    t: f32,
    stopped: bool,
    palette: usize,
    /// The rows of cells the picture covers.
    rows: u16,
    stones: Vec<Stone>,
    /// When the liquid was last stirred, and stones last dropped.
    stirred: f32,
    dropped: Vec<[f32; 2]>,
}

impl Plasma {
    pub fn new() -> Self {
        Self { t: 0.0, stopped: false, palette: 0, rows: 22, stones: Vec::new(), stirred: 0.0, dropped: Vec::new() }
    }

    /// Where the plasma's eye is at `t`, in picture heights from its
    /// middle, for a picture `aspect` heights wide.
    fn eye(t: f32, aspect: f32) -> [f32; 2] {
        [0.42 * aspect * (t * 0.31).sin(), 0.36 * (t * 0.43 + 1.0).sin()]
    }

    /// The plasma's value at (x, y), in picture heights from its middle:
    /// four waves, one round the eye, and the stones' rings, about 0 to 1.
    fn value(&self, [x, y]: [f32; 2], eye: [f32; 2]) -> f32 {
        let t = self.t;
        let d = ((x - eye[0]).powi(2) + (y - eye[1]).powi(2)).sqrt();
        let mut v = (x * 6.2 + t * 0.9).sin() + (y * 5.4 - t * 0.7).sin() + ((x + y) * 4.4 + t * 0.5).sin() + (d * 11.0 - t * 1.6).sin();
        for stone in &self.stones {
            let age = t - stone.born;
            let off = ((x - stone.at[0]).powi(2) + (y - stone.at[1]).powi(2)).sqrt() - age * RING_SPEED;
            v += 2.4 * (-(off / 0.06).powi(2)).exp() * (1.0 - age / RING_LIFE).max(0.0);
        }
        v * 0.125 + 0.5
    }

    /// Where a value falls in the palette, which turns as time goes.
    fn index(&self, value: f32) -> f32 {
        ((value * BANDS + self.t * 0.07) * COLORS).floor() / COLORS
    }

    /// The picture's size in pixels over `rows` rows of cells and
    /// `width` × `height` of the screen's pixels: about [`PIXELS`] of them,
    /// a whole number to each row, so that where each row is a picture of
    /// its own (bare cells) they're whole.
    pub fn size(rows: u16, width: f32, height: f32) -> (u32, u32) {
        let rows = f32::from(rows.max(1));
        let k = (width * height / PIXELS).sqrt().max(1.0);
        let per_row = (height / rows / k).round().max(1.0);
        let h = rows * per_row;
        (((h * width / height).round()).max(1.0) as u32, h as u32)
    }

    /// The picture at the plasma's time, `w` × `h` pixels.
    fn paint(&self, w: u32, h: u32) -> Image {
        let palette = &PALETTES[self.palette];
        let (fw, fh) = (w as f32, h as f32);
        let aspect = fw / fh;
        let eye = Self::eye(self.t, aspect);
        let fade = fh * FADE;
        let mut px = Vec::with_capacity(w as usize * h as usize * 4);
        for j in 0..h {
            let y = 0.5 - (j as f32 + 0.5) / fh;
            let edge_y = (j.min(h - 1 - j) as f32 / fade).min(1.0);
            for i in 0..w {
                let x = (i as f32 + 0.5) / fh - aspect * 0.5;
                let edge = edge_y.min((i.min(w - 1 - i) as f32 / fade).min(1.0));
                let [r, g, b, a] = palette.at(self.index(self.value([x, y], eye)));
                let a = a as f32 * edge * edge * (3.0 - 2.0 * edge);
                px.extend([r, g, b, a.round() as u8]);
            }
        }
        Image::rgba(w, h, px)
    }
}

impl Default for Plasma {
    fn default() -> Self {
        Self::new()
    }
}

impl Demo for Plasma {
    fn shows(&self) -> Shows {
        Shows::Pictures
    }

    fn interval(&self) -> Duration {
        INTERVAL
    }

    fn cells(&mut self, _cols: u16, rows: u16) {
        self.rows = rows;
    }

    fn words(&self) -> u16 {
        2
    }

    fn frame(&mut self, dt: f32, w: f32, h: f32) -> Frame {
        let mut effects = Vec::new();
        let aspect = w / h;
        for at in std::mem::take(&mut self.dropped) {
            let stone = Stone { at: [at[0] / h, at[1] / h], born: self.t };
            let color = PALETTES[self.palette].rgb(self.index(self.value(stone.at, Self::eye(self.t, aspect))));
            effects.push(Fx::new(Kind::Ripple, at, w * 0.12, 0.55));
            effects.push(Fx::new(Kind::Burst, at, w * 0.05, 0.4).color(color).quiet());
            self.stones.push(stone);
        }
        if !self.stopped {
            self.t += dt;
            self.stones.retain(|s| self.t - s.born < RING_LIFE);
            if self.t - self.stirred >= STIR {
                self.stirred = self.t;
                // Along the way the eye is going.
                let (now, soon) = (Self::eye(self.t, aspect), Self::eye(self.t + 0.2, aspect));
                let dir = [soon[0] - now[0], soon[1] - now[1]];
                let len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt().max(1e-4);
                effects.push(Fx::new(Kind::Stir, [now[0] * h, now[1] * h], w * 0.25, 0.35).dir([dir[0] / len, dir[1] / len]).quiet());
            }
        }
        let (pw, ph) = Self::size(self.rows, w, h);
        let image = self.paint(pw, ph);
        let name = PALETTES[self.palette].name;
        let words = if self.stopped {
            vec![Line::new("Stopped · space starts it again · q quits", WORDS), Line::new("", WORDS)]
        } else {
            vec![
                Line::new("Plasma, made in code each frame and sent with kitty's graphics protocol", WORDS),
                Line::new(format!("click to drop a stone · ← → colors: {name} · space stops it · q quits"), WORDS),
            ]
        };
        Frame { show: Show::Picture(image), effects, words }
    }

    fn key(&mut self, key: KeyCode) {
        let n = PALETTES.len();
        match key {
            KeyCode::Left | KeyCode::Char('h') => self.palette = (self.palette + n - 1) % n,
            KeyCode::Right | KeyCode::Char('l') => self.palette = (self.palette + 1) % n,
            KeyCode::Char(' ') => self.stopped = !self.stopped,
            _ => {}
        }
    }

    fn click(&mut self, at: [f32; 2]) {
        self.dropped.push(at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picture_has_whole_pixels_to_a_row_and_the_stages_shape() {
        let (w, h) = Plasma::size(40, 1140.0, 760.0);
        assert_eq!(h % 40, 0, "{h}");
        assert!(((w as f32 / h as f32) - 1140.0 / 760.0).abs() < 0.02);
        assert!((25_000..60_000).contains(&(w * h)), "{w} × {h}");
        assert_eq!(Plasma::size(6, 200.0, 120.0).1 % 6, 0, "small, it still has a pixel a row");
    }

    #[test]
    fn its_edges_fade_to_the_glass_and_its_middle_shows() {
        let mut plasma = Plasma::new();
        plasma.cells(100, 30);
        let frame = plasma.frame(0.5, 950.0, 570.0);
        let image = frame.picture().expect("a picture");
        let alpha = |x: u32, y: u32| image.data[((y * image.width + x) * 4 + 3) as usize];
        assert_eq!(alpha(0, 0), 0);
        assert!(alpha(image.width / 2, image.height / 2) > 128, "the rainbow is nearly opaque");
        assert_eq!(image.height % 30, 0);
    }

    #[test]
    fn it_moves_on_but_stopped_its_picture_is_the_same() {
        let mut plasma = Plasma::new();
        let a = plasma.frame(0.1, 800.0, 400.0).picture().cloned();
        let b = plasma.frame(0.1, 800.0, 400.0).picture().cloned();
        assert_ne!(a, b);
        plasma.key(KeyCode::Char(' '));
        let c = plasma.frame(0.1, 800.0, 400.0).picture().cloned();
        let d = plasma.frame(0.1, 800.0, 400.0).picture().cloned();
        assert_eq!(c, d, "nothing moves, so nothing need be sent");
    }

    #[test]
    fn a_stone_rings_out_and_ripples_the_liquid_where_it_fell() {
        let mut plasma = Plasma::new();
        plasma.frame(0.1, 800.0, 400.0);
        plasma.click([100.0, -50.0]);
        let frame = plasma.frame(0.1, 800.0, 400.0);
        let ripple = frame.effects.iter().find(|f| f.kind == Kind::Ripple).expect("a ripple");
        assert_eq!(ripple.at, [100.0, -50.0]);
        assert!(frame.effects.iter().any(|f| f.kind == Kind::Burst && f.color.is_some()));
        // Its ring goes out from the stone, raising the plasma there and
        // nowhere else.
        let stone = plasma.stones[0];
        let calm = Plasma { stones: Vec::new(), ..plasma.clone() };
        let eye = Plasma::eye(plasma.t, 2.0);
        let raised = |d: f32| plasma.value([stone.at[0] + d, stone.at[1]], eye) - calm.value([stone.at[0] + d, stone.at[1]], eye);
        let front = (plasma.t - stone.born) * RING_SPEED;
        assert!(raised(front) > 0.2 && raised(front + 0.3).abs() < 0.01, "{} {}", raised(front), raised(front + 0.3));
        for _ in 0..(RING_LIFE * 10.0) as usize + 1 {
            plasma.frame(0.1, 800.0, 400.0);
        }
        assert!(plasma.stones.is_empty(), "and dies away");
    }

    #[test]
    fn the_liquid_is_stirred_along_its_eyes_way_now_and_then() {
        let mut plasma = Plasma::new();
        let stirs: usize = (0..300).map(|_| plasma.frame(1.0 / 30.0, 800.0, 400.0).effects.iter().filter(|f| f.kind == Kind::Stir && f.dir.is_some()).count()).sum();
        assert_eq!(stirs, (10.0 / STIR) as usize);
    }

    #[test]
    fn the_arrows_go_round_the_palettes() {
        let mut plasma = Plasma::new();
        plasma.key(KeyCode::Left);
        assert_eq!(plasma.palette, PALETTES.len() - 1);
        plasma.key(KeyCode::Right);
        plasma.key(KeyCode::Right);
        assert_eq!(plasma.palette, 1);
        // Each palette goes round smoothly, ending where it began.
        for p in &PALETTES {
            let (a, b) = (p.at(0.0), p.at(0.9999));
            assert!(a.iter().zip(b).all(|(x, y)| (i32::from(*x) - i32::from(y)).abs() <= 2), "{}", p.name);
        }
    }
}
