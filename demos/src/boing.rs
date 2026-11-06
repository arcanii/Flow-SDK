//! Boing: a ball checkered red and white, turning about a tilted axis,
//! bounces from side to side before a purple grid that runs back into a
//! floor, its shadow on the wall behind it, after the Amiga's demo of 1984.
//! Where it lands Flow's liquid ripples and splashes red with a boing, and
//! where it meets a side the liquid is stirred with a thud. A click, or the arrow keys, send it
//! the other way; space stops it.

use std::f32::consts::{PI, TAU};

use flow_sdk::fx::{Kind, Sound};
use flow_sdk::input::KeyCode;
use flow_sdk::scene::{add, apply, compose, lit_as, mul, rgb, rotation, Color, Corner, Scene};
use flow_sdk::stage::Fx;

use crate::{Demo, Frame, Line, Show};

const RED: u32 = 0xe8231c;
const WHITE: u32 = 0xf4f4f4;
const GRID: u32 = 0xb04ad0;
const WORDS: u32 = 0x9aa0b4;
/// The ball's bands round its axis, and from pole to pole.
const AROUND: usize = 16;
const DOWN: usize = 8;
/// How far its axis leans right of upright, radians.
const TILT: f32 = 0.3;
/// Seconds from one landing to the next.
const BOUNCE: f32 = 1.45;
/// Seconds to go from one side to the other.
const CROSS: f32 = 3.8;
/// Radians a second it turns.
const SPIN: f32 = 2.4;
/// Squares deep the floor runs.
const FLOOR_ROWS: f32 = 4.0;

pub struct Boing {
    /// Across, from 0 at the left to 1 at the right.
    x: f32,
    /// The way it's going: 1 right, -1 left.
    dir: f32,
    /// Through a bounce, from one landing (0) to the next (1).
    phase: f32,
    spin: f32,
    stopped: bool,
    /// Where its center was last drawn, across the picture.
    shown_x: f32,
}

/// Where the room's parts are in a picture: the wall's squares, the floor
/// running back from its foot, and where the ball goes.
struct Room {
    square: f32,
    line: f32,
    /// Squares either side of the middle.
    half: i32,
    top: f32,
    foot: f32,
    bottom: f32,
    /// Where the floor's lines meet, and how far the eye is from the wall.
    vanish: f32,
    eye: f32,
    radius: f32,
    /// The height it lands at, the highest its center goes, and how far
    /// its center goes each way.
    ground: f32,
    high: f32,
    reach: f32,
}

impl Room {
    fn new(w: f32, h: f32) -> Self {
        let square = h / 8.0;
        let line = (h * 0.0045).max(1.5);
        let half = ((w * 0.5 - square * 0.3) / square).floor().max(1.0) as i32;
        let foot = -h * 0.5 + h * 0.24;
        let top = foot + ((h * 0.5 - square * 0.35 - foot) / square).floor().max(1.0) * square;
        let bottom = -h * 0.5 + h * 0.015;
        let vanish = h * 0.2;
        // The floor's last line is at the bottom, FLOOR_ROWS squares out.
        let spread = (vanish - bottom) / (vanish - foot);
        let eye = FLOOR_ROWS * square / (1.0 - 1.0 / spread);
        let radius = h * 0.19;
        let ground = vanish - (vanish - foot) * eye / (eye - 1.8 * square);
        Self {
            square,
            line,
            half,
            top,
            foot,
            bottom,
            vanish,
            eye,
            radius,
            ground,
            high: top - radius * 1.05,
            reach: (w * 0.5 - radius - square * 0.3).max(0.0),
        }
    }

    /// How much bigger the floor shows `d` out from the wall.
    fn nearer(&self, d: f32) -> f32 {
        self.eye / (self.eye - d)
    }
}

impl Boing {
    pub fn new() -> Self {
        Self { x: 0.15, dir: 1.0, phase: 0.35, spin: 0.0, stopped: false, shown_x: 0.0 }
    }
}

impl Default for Boing {
    fn default() -> Self {
        Self::new()
    }
}

impl Demo for Boing {
    fn words(&self) -> u16 {
        1
    }

    fn frame(&mut self, dt: f32, w: f32, h: f32) -> Frame {
        let room = Room::new(w, h);
        let r = room.radius;
        let mut effects = Vec::new();
        let center_y = |phase: f32| room.ground + r + (room.high - room.ground - r).max(0.0) * 4.0 * phase * (1.0 - phase);
        if !self.stopped {
            self.x += self.dir * dt / CROSS;
            let side = if self.x > 1.0 {
                self.x = 2.0 - self.x;
                Some(1.0)
            } else if self.x < 0.0 {
                self.x = -self.x;
                Some(-1.0)
            } else {
                None
            };
            if let Some(side) = side {
                // It hits the side it was going to, and pushes the liquid on.
                self.dir = -side;
                effects.push(Fx::new(Kind::Stir, [side * (room.reach + r), center_y(self.phase)], r, 0.5).dir([side, 0.0]).sound(Sound::Thud));
            }
            self.phase += dt / BOUNCE;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
                let at = [room.reach * (self.x * 2.0 - 1.0), room.ground];
                effects.push(Fx::new(Kind::Ripple, at, r * 1.6, 0.55).sound(Sound::Boing));
                effects.push(Fx::new(Kind::Burst, at, r * 0.7, 0.35).color(RED).quiet());
            }
            self.spin = (self.spin + SPIN * self.dir * dt) % TAU;
        }
        let (cx, cy) = (room.reach * (self.x * 2.0 - 1.0), center_y(self.phase));
        self.shown_x = cx;

        let mut scene = Scene::new(w, h);
        draw_room(&mut scene, &room);
        // Its shadow on the wall, behind the grid's lines.
        let shadow: Vec<[f32; 3]> = (0..40).map(|i| i as f32 / 40.0 * TAU).map(|a| [cx + r * 0.32 + r * a.cos(), cy - r * 0.06 + r * a.sin(), -3.0]).collect();
        scene.flat([0.0, 0.0, 0.0, 0.35], &shadow, [0.0, 0.0, 1.0]);
        draw_ball(&mut scene, [cx, cy, r + 6.0], r, self.spin);

        let words = if self.stopped { "Stopped · space starts it again · q quits" } else { "Boing, after the Amiga's demo of 1984 · click or ← → to send it back · space stops it · q quits" };
        Frame { show: Show::Scene(scene), effects, words: vec![Line::new(words, WORDS)] }
    }

    fn key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Left | KeyCode::Char('h') => self.dir = -1.0,
            KeyCode::Right | KeyCode::Char('l') => self.dir = 1.0,
            KeyCode::Char(' ') => self.stopped = !self.stopped,
            _ => {}
        }
    }

    fn click(&mut self, at: [f32; 2]) {
        self.dir = if at[0] > self.shown_x { 1.0 } else { -1.0 };
        self.stopped = false;
    }
}

/// The wall's squares, and the floor's running back toward the eye.
fn draw_room(scene: &mut Scene, room: &Room) {
    let color = lit_as(rgb(GRID), [0.0, 0.0, 1.0]);
    let quad = |scene: &mut Scene, points: [[f32; 2]; 4]| scene.flat(color, &points.map(|[x, y]| [x, y, 0.0]), [0.0, 0.0, 1.0]);
    let (s, l) = (room.square, room.line * 0.5);
    let edge = room.half as f32 * s;
    for k in -room.half..=room.half {
        let x = k as f32 * s;
        quad(scene, [[x - l, room.foot], [x + l, room.foot], [x + l, room.top], [x - l, room.top]]);
        // On the floor, the line runs out toward the eye, wider as it comes.
        let far = room.nearer(FLOOR_ROWS * s);
        quad(scene, [[x - l, room.foot], [x + l, room.foot], [x * far + l * far, room.bottom], [x * far - l * far, room.bottom]]);
    }
    let rows = ((room.top - room.foot) / s).round() as i32;
    for row in 0..=rows {
        let y = room.foot + row as f32 * s;
        quad(scene, [[-edge - l, y - l], [edge + l, y - l], [edge + l, y + l], [-edge - l, y + l]]);
    }
    for row in 1..=FLOOR_ROWS as i32 {
        let k = room.nearer(row as f32 * s);
        let y = room.vanish - (room.vanish - room.foot) * k;
        let (x, t) = (edge * k, l * k * 0.8);
        quad(scene, [[-x - t, y - t], [x + t, y - t], [x + t, y + t], [-x - t, y + t]]);
    }
}

/// The ball at `center`, `r` across from it, turned `spin` about its
/// leaning axis, its bands of red and white lit smoothly.
fn draw_ball(scene: &mut Scene, center: [f32; 3], r: f32, spin: f32) {
    let turn = compose(rotation([0.0, 0.0, 1.0], -TILT), rotation([0.0, 1.0, 0.0], spin));
    let point = |i: usize, j: usize| {
        let (down, around) = (PI * i as f32 / DOWN as f32, TAU * j as f32 / AROUND as f32);
        apply(turn, [down.sin() * around.cos(), down.cos(), down.sin() * around.sin()])
    };
    let (red, white): (Color, Color) = (rgb(RED), rgb(WHITE));
    for i in 0..DOWN {
        for j in 0..AROUND {
            let mut corners: Vec<Corner> = Vec::with_capacity(4);
            for (a, b) in [(i, j), (i, j + 1), (i + 1, j + 1), (i + 1, j)] {
                let n = point(a, b);
                let at = add(center, mul(n, r));
                // The poles' bands are triangles.
                if corners.last().is_none_or(|c: &Corner| c.at != at) {
                    corners.push(Corner { at, normal: n });
                }
            }
            // Bands turned away from the eye are hidden anyway.
            if corners.iter().all(|c| c.normal[2] < -0.05) {
                continue;
            }
            scene.polygon(if (i + j) % 2 == 0 { red } else { white }, &corners);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ball_lands_on_the_floor_and_its_room_fits_the_picture() {
        let (w, h) = (1500.0, 900.0);
        let room = Room::new(w, h);
        assert!(room.foot > room.ground && room.ground > room.bottom, "it lands on the floor, in front of the wall");
        assert!(room.high > room.ground + room.radius, "and bounces up");
        assert!(room.reach + room.radius < w * 0.5);
        assert!((room.vanish - (room.vanish - room.foot) * room.nearer(FLOOR_ROWS * room.square) - room.bottom).abs() < 0.01, "the floor's last line is at the bottom");
    }

    #[test]
    fn it_turns_back_at_a_side_and_stirs_the_liquid_there() {
        let mut boing = Boing { x: 0.99, dir: 1.0, phase: 0.2, spin: 0.0, stopped: false, shown_x: 0.0 };
        let frame = boing.frame(0.1, 1200.0, 700.0);
        assert_eq!(boing.dir, -1.0);
        assert!(frame.effects.iter().any(|f| f.kind == Kind::Stir && f.dir == Some([1.0, 0.0])));
        assert!(frame.scene().expect("a scene").triangles() > 100);
    }

    #[test]
    fn a_landing_ripples_where_it_lands() {
        let mut boing = Boing { x: 0.5, dir: 1.0, phase: 0.99, spin: 0.0, stopped: false, shown_x: 0.0 };
        let frame = boing.frame(0.05, 1200.0, 700.0);
        let ripple = frame.effects.iter().find(|f| f.kind == Kind::Ripple).expect("it landed");
        assert!(ripple.at[0].abs() < 30.0 && ripple.at[1] < 0.0);
    }
}
