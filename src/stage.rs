//! Where a scene ([`crate::scene`]) goes: the terminal's cells above the
//! rows a program keeps for its words, as many as one placement may cover
//! (in Flow, 200 cells a side, 4096 pixels a side and 8 million pixels in
//! all), centered. Each frame sends the scene again as the same object
//! ([`crate::model::register`]), which the terminal shows in place of the
//! last, and the first places it, scaled up by the tenth the terminal leaves
//! round a model ([`crate::model::FILL`]). A frame like the last isn't sent.
//!
//! The scene is drawn in the screen's pixels, fractions of a pixel and all,
//! and squashed into the terminal's picture, which has a cell's whole
//! pixels per cell (`CSI 16 t`). Points of the picture and the screen's
//! cells map to each other both ways, for clicks and for effects ([`Fx`]).
//! A stage made [`Stage::packed`] sends each frame packed into whole
//! numbers ([`Scene::glb_packed`]), half the size, for Flow.

use std::borrow::Cow;

use crate::fx::{Effect, FxSound, Kind, Sound};
use crate::model::{self, Format, Look, Place};
use crate::scene::Scene;
use crate::term::Size;

/// Flow's limits on a placement ([`model::MAX_SPAN`], the picture's sides,
/// and somewhat under [`model::MAX_PIXELS`]).
const MAX_CELLS: u16 = model::MAX_SPAN;
const MAX_SIDE: u32 = model::MAX_SIDE;
const MAX_PIXELS: u32 = 8_000_000;
/// The smallest stage worth showing.
const MIN_COLS: u32 = 16;
const MIN_ROWS: u32 = 6;

/// A scene's place on the screen, which sends each frame's scene there.
#[derive(Debug, Clone, PartialEq)]
pub struct Stage {
    /// Its left column, from 0.
    pub left: u16,
    /// Its top row, from 0.
    pub top: u16,
    /// How many cells wide it is.
    pub cols: u16,
    /// How many cells high it is.
    pub rows: u16,
    /// A cell's size on the screen, in pixels.
    pub cell: (f32, f32),
    /// A cell's whole pixels: the terminal's picture has these per cell.
    pub whole: (u16, u16),
    /// The object every frame is.
    id: u32,
    /// Named in each GLB as the program that made it: a name of the
    /// program's own, or one it was given at run time (from C, say).
    generator: Cow<'static, str>,
    /// Frames go packed into whole numbers.
    packed: bool,
    placed: bool,
    /// The last frame sent, which a still scene needn't send again.
    last: Vec<u8>,
}

impl Stage {
    /// The stage for a terminal `size` whose cells have `whole` pixels
    /// ([`crate::detect::Caps::cell`]), keeping `words` rows below it; none
    /// if it would be too small (under 16 × 6 cells). Its object is id 1.
    pub fn new(size: Size, whole: Option<(u16, u16)>, words: u16) -> Option<Self> {
        let cell = size.cell.or(whole.map(|(w, h)| (w as f32, h as f32)))?;
        let whole = whole.unwrap_or((cell.0 as u16, cell.1 as u16));
        let (ww, wh) = (u32::from(whole.0.max(1)), u32::from(whole.1.max(1)));
        let rows_free = size.rows.saturating_sub(words);
        let mut cols = u32::from(size.cols.min(MAX_CELLS)).min(MAX_SIDE / ww);
        let mut rows = u32::from(rows_free.min(MAX_CELLS)).min(MAX_SIDE / wh);
        if cols * ww * rows * wh > MAX_PIXELS {
            let k = (MAX_PIXELS as f32 / (cols * ww * rows * wh) as f32).sqrt();
            (cols, rows) = ((cols as f32 * k) as u32, (rows as f32 * k) as u32);
        }
        if cols < MIN_COLS || rows < MIN_ROWS {
            return None;
        }
        let (cols, rows) = (cols as u16, rows as u16);
        Some(Self { left: (size.cols - cols) / 2, top: (rows_free - rows) / 2, cols, rows, cell, whole, id: 1, generator: Cow::Borrowed("flow-sdk"), packed: false, placed: false, last: Vec::new() })
    }

    /// Its object's id, in place of 1.
    pub fn id(self, id: u32) -> Self {
        Self { id, ..self }
    }

    /// Names the program in each frame's GLB (glTF's `asset.generator`).
    pub fn made_by(self, generator: impl Into<Cow<'static, str>>) -> Self {
        Self { generator: generator.into(), ..self }
    }

    /// Sends each frame packed into whole numbers ([`Scene::glb_packed`],
    /// glTF's KHR_mesh_quantization), half the size, or not: packed for
    /// Flow, which reads them (as of 0.1.27), not for every terminal.
    pub fn packed(self, packed: bool) -> Self {
        Self { packed, ..self }
    }

    /// Whether frames go packed.
    pub fn is_packed(&self) -> bool {
        self.packed
    }

    /// The picture's width on the screen, in pixels: a scene's for it.
    pub fn width(&self) -> f32 {
        self.cols as f32 * self.cell.0
    }

    /// The picture's height on the screen, in pixels: a scene's for it.
    pub fn height(&self) -> f32 {
        self.rows as f32 * self.cell.1
    }

    /// The first row below the picture.
    pub fn below(&self) -> u16 {
        self.top + self.rows
    }

    /// How the screen's pixels go into the picture's.
    fn squash(&self) -> (f32, f32) {
        (self.whole.0 as f32 / self.cell.0, self.whole.1 as f32 / self.cell.1)
    }

    /// The screen cell (column, row) a point of the picture is in.
    pub fn cell_at(&self, [x, y]: [f32; 2]) -> (u16, u16) {
        let col = ((x + self.width() * 0.5) / self.cell.0).clamp(0.0, self.cols as f32 - 1.0);
        let row = ((self.height() * 0.5 - y) / self.cell.1).clamp(0.0, self.rows as f32 - 1.0);
        (self.left + col as u16, self.top + row as u16)
    }

    /// The point of the picture at the middle of a screen cell, if it's on
    /// the picture.
    pub fn point_at(&self, col: u16, row: u16) -> Option<[f32; 2]> {
        let (c, r) = (col.checked_sub(self.left)?, row.checked_sub(self.top)?);
        (c < self.cols && r < self.rows).then(|| [(c as f32 + 0.5) * self.cell.0 - self.width() * 0.5, self.height() * 0.5 - (r as f32 + 0.5) * self.cell.1])
    }

    /// What shows `scene`, unless it's the last frame again: the object,
    /// and the first time where it goes.
    pub fn show(&mut self, scene: &Scene, out: &mut Vec<u8>) {
        let glb = if self.packed { scene.glb_packed(self.squash(), &self.generator) } else { scene.glb_by(self.squash(), &self.generator) };
        if self.placed && glb == self.last {
            return;
        }
        out.extend(model::register(self.id, Format::Glb, &glb, Some("scene.glb")).as_bytes());
        self.last = glb;
        if !self.placed {
            let place = Place::over(self.id, (self.top, self.left), (self.cols, self.rows)).look(Look { scale: Some(model::FILL), ..Look::default() });
            out.extend(place.escape().as_bytes());
            self.placed = true;
        }
    }

    /// Takes the picture away.
    pub fn clear(&self, out: &mut Vec<u8>) {
        out.extend(model::delete(self.id).as_bytes());
    }

    /// An effect for Flow's liquid at a point of the picture, in cells.
    pub fn effect(&self, fx: &Fx) -> Effect {
        Effect {
            kind: fx.kind,
            at: self.cell_at(fx.at),
            to: fx.to.map(|to| self.cell_at(to)),
            color: fx.color,
            // The terminal's rows go down.
            dir: fx.dir.map(|[x, y]| (x, -y)),
            sound: fx.sound,
            size: Some((fx.size / self.cell.0.max(1.0)).clamp(0.5, 40.0)),
            strength: fx.strength,
            volume: 1.0,
        }
    }
}

/// An effect at a point of a stage's picture, in its pixels from its
/// middle, y up, as a scene has them; [`Stage::effect`] puts it in cells.
#[derive(Debug, Clone, PartialEq)]
pub struct Fx {
    /// What it does.
    pub kind: Kind,
    /// Where it's set off.
    pub at: [f32; 2],
    /// Where a bolt ends.
    pub to: Option<[f32; 2]>,
    /// How far it reaches, in pixels.
    pub size: f32,
    /// How hard, 0 to 1.
    pub strength: f32,
    /// 0xRRGGBB.
    pub color: Option<u32>,
    /// Which way a stir pushes, y up.
    pub dir: Option<[f32; 2]>,
    /// What it sounds like.
    pub sound: FxSound,
}

impl Fx {
    /// An effect of `kind` at `at`, reaching `size` pixels, as hard as
    /// `strength`, sounding as its kind does.
    pub fn new(kind: Kind, at: [f32; 2], size: f32, strength: f32) -> Self {
        Self { kind, at, to: None, size, strength, color: None, dir: None, sound: FxSound::Kind }
    }

    /// Ending at `to`, as a bolt does.
    pub fn to(self, to: [f32; 2]) -> Self {
        Self { to: Some(to), ..self }
    }

    /// In `color`, 0xRRGGBB, where the liquid takes color.
    pub fn color(self, color: u32) -> Self {
        Self { color: Some(color), ..self }
    }

    /// Pushing along `dir`, y up, as a stir does.
    pub fn dir(self, dir: [f32; 2]) -> Self {
        Self { dir: Some(dir), ..self }
    }

    /// One of Flow's sounds in place of its kind's.
    pub fn sound(self, sound: Sound) -> Self {
        Self { sound: FxSound::Named(sound), ..self }
    }

    /// No sound at all.
    pub fn quiet(self) -> Self {
        Self { sound: FxSound::Quiet, ..self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stage_keeps_inside_flows_limits_and_centers() {
        let size = Size { cols: 240, rows: 70, cell: Some((18.4, 38.2)) };
        let stage = Stage::new(size, Some((18, 38)), 2).expect("big enough");
        assert!(stage.cols <= MAX_CELLS && u32::from(stage.cols) * 18 <= MAX_SIDE);
        assert!(u32::from(stage.cols) * 18 * u32::from(stage.rows) * 38 <= MAX_PIXELS);
        assert_eq!(stage.left, (240 - stage.cols) / 2);
        assert!(stage.below() <= 68, "the words' rows stay free");
        assert_eq!(Stage::new(Size { cols: 12, rows: 5, cell: Some((9.0, 18.0)) }, Some((9, 18)), 1), None);
    }

    #[test]
    fn a_click_and_an_effect_land_in_the_same_cell() {
        let stage = Stage::new(Size { cols: 80, rows: 24, cell: Some((9.5, 19.0)) }, Some((9, 19)), 1).expect("big enough");
        let p = stage.point_at(30, 10).expect("on the picture");
        assert_eq!(stage.cell_at(p), (30, 10));
        assert_eq!(stage.point_at(30, 23), None, "the words' row isn't the picture");
        let fx = Fx::new(Kind::Stir, p, 90.0, 0.5).color(0xe02020).dir([1.0, 1.0]).sound(Sound::Thud);
        assert_eq!(stage.effect(&fx).escape(), "\x1b]7731;fx=stir;at=30,10;color=e02020;dir=1.00,-1.00;sound=thud;size=9.5;strength=0.50\x07");
    }

    #[test]
    fn a_scene_is_sent_whole_each_frame_and_placed_once() {
        let mut stage = Stage::new(Size { cols: 80, rows: 24, cell: Some((9.5, 19.0)) }, Some((9, 19)), 1).expect("big enough").made_by("a-test");
        let mut scene = Scene::new(stage.width(), stage.height());
        scene.flat(crate::scene::rgb(0xff0000), &[[0.0, 0.0, 0.0], [50.0, 0.0, 0.0], [0.0, 50.0, 0.0]], [0.0, 0.0, 1.0]);
        let mut first = Vec::new();
        stage.show(&scene, &mut first);
        let first = String::from_utf8(first).unwrap();
        assert!(first.starts_with("\x1b_ratty;g;r;id=1;fmt=glb;source=payload;more=0;name=scene.glb;"));
        assert!(first.ends_with(&format!("\x1b_ratty;g;p;id=1;row={};col={};w={};h={};scale=1.1111112\x1b\\", stage.top + 11, stage.left + 40, stage.cols, stage.rows)));
        let mut again = Vec::new();
        stage.show(&scene, &mut again);
        assert!(again.is_empty(), "the same frame isn't sent again");
        scene.flat(crate::scene::rgb(0x00ff00), &[[0.0, 0.0, 0.0], [-50.0, 0.0, 0.0], [0.0, -50.0, 0.0]], [0.0, 0.0, 1.0]);
        stage.show(&scene, &mut again);
        let again = String::from_utf8(again).unwrap();
        assert!(again.starts_with("\x1b_ratty;g;r;id=1;") && !again.contains("ratty;g;p"), "a new frame takes the last one's place");
        let mut gone = Vec::new();
        stage.clone().id(7).clear(&mut gone);
        assert_eq!(gone, b"\x1b_ratty;g;d;id=7\x1b\\");
    }

    #[test]
    fn a_packed_stage_sends_its_frames_packed() {
        let size = Size { cols: 80, rows: 24, cell: Some((9.5, 19.0)) };
        let mut stage = Stage::new(size, Some((9, 19)), 1).expect("big enough").packed(true);
        assert!(stage.is_packed() && !Stage::new(size, Some((9, 19)), 1).unwrap().is_packed());
        let mut scene = Scene::new(stage.width(), stage.height());
        scene.flat(crate::scene::rgb(0xff0000), &[[0.0, 0.0, 0.0], [50.0, 0.0, 0.0], [0.0, 50.0, 0.0]], [0.0, 0.0, 1.0]);
        let mut sent = Vec::new();
        stage.show(&scene, &mut sent);
        let text = String::from_utf8(sent).unwrap();
        let glb = model::register(1, Format::Glb, &scene.glb_packed(stage.squash(), "flow-sdk"), Some("scene.glb"));
        assert!(text.starts_with(&glb), "the packed frame");
    }
}
