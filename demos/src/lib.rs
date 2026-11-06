//! Demos of what Flow draws, each setting its liquid moving where things
//! happen: scenes in 3D over the liquid, each frame sent with the Ratty
//! graphics protocol (the bouncing ball, the rolling cube, the vector
//! display's glowing lines, made for the CRT, and a day passing over hills
//! whose sky is Flow's glass, washed with the sky's color); a picture made
//! in code each frame and sent with kitty's (the plasma); a page of
//! teletext written in the screen's own cells, its lines doubled and its
//! mosaics drawn by Flow; a keyboard in cells that plays Flow's sounds as a
//! program asks for them; and a balloon Flow reads from a file once the
//! user says it may. Like the games, they use none of Flow's crates: the
//! scenes, the stage they're shown on, the pictures, the models, the
//! effects and the terminal are `flow-sdk`'s.

pub mod app;
pub mod balloon;
pub mod boing;
pub mod cells;
pub mod clock;
pub mod cube;
pub mod hangup;
pub mod mosaic;
pub mod pictures;
pub mod plasma;
pub mod sounds;
pub mod teletext;
pub mod tint;
pub mod vector;

use std::ops::Range;
use std::time::Duration;

use flow_sdk::input::KeyCode;
use flow_sdk::picture::Image;
use flow_sdk::scene::Scene;
use flow_sdk::stage::Fx;
use flow_sdk::term::Size;

/// How often a frame goes out, at most, unless a demo wants fewer.
pub const FRAME: Duration = Duration::from_micros(16_667);

/// A line of words below the picture, centered.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub text: String,
    pub color: u32,
    pub bold: bool,
    /// A stretch of it, by characters, lit in a color of its own and bold:
    /// the one shown of a row of choices, say.
    pub lit: Option<(Range<usize>, u32)>,
}

impl Line {
    pub fn new(text: impl Into<String>, color: u32) -> Self {
        Self { text: text.into(), color, bold: false, lit: None }
    }

    pub fn bold(self) -> Self {
        Self { bold: true, ..self }
    }

    /// With the characters `range` lit in `color`, bold.
    pub fn lit(self, range: Range<usize>, color: u32) -> Self {
        Self { lit: Some((range, color)), ..self }
    }

    /// Its stretches of words, each with its color and whether it's bold:
    /// the whole line, or what comes before the lit stretch, the lit
    /// stretch, and what comes after.
    pub fn spans(&self) -> Vec<(String, u32, bool)> {
        let Some((range, color)) = &self.lit else { return vec![(self.text.clone(), self.color, self.bold)] };
        let part = |r: Range<usize>| self.text.chars().skip(r.start).take(r.len()).collect::<String>();
        let len = self.text.chars().count();
        let (start, end) = (range.start.min(len), range.end.clamp(range.start.min(len), len));
        [(part(0..start), self.color, self.bold), (part(start..end), *color, true), (part(end..len), self.color, self.bold)].into_iter().filter(|s| !s.0.is_empty()).collect()
    }
}

/// What a frame on the stage is.
pub enum Show {
    /// A 3D scene, sent as a model with the Ratty graphics protocol.
    Scene(Scene),
    /// A picture, sent with kitty's graphics protocol over the stage's
    /// cells ([`pictures::Easel`]).
    Picture(Image),
}

/// What a frame shows and sets off.
pub struct Frame {
    pub show: Show,
    pub effects: Vec<Fx>,
    pub words: Vec<Line>,
}

impl Frame {
    /// Its scene, if it's one.
    pub fn scene(&self) -> Option<&Scene> {
        match &self.show {
            Show::Scene(scene) => Some(scene),
            Show::Picture(_) => None,
        }
    }

    /// Its picture, if it's one.
    pub fn picture(&self) -> Option<&Image> {
        match &self.show {
            Show::Picture(image) => Some(image),
            Show::Scene(_) => None,
        }
    }
}

/// What a demo on the stage shows, and so what the terminal has to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shows {
    Scenes,
    Pictures,
}

/// What a demo is told as it starts: whether it plays in Flow (whose
/// sounds and glass are Flow's alone, so it can say so where they aren't),
/// and whether it may set the liquid off (not with `--no-effects`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Told {
    pub flow: bool,
    pub effects: bool,
}

/// A demo drawn on a stage over the liquid, a frame at a time, with words
/// below it.
pub trait Demo {
    fn shows(&self) -> Shows {
        Shows::Scenes
    }
    fn told(&mut self, _told: Told) {}
    /// The least time between frames.
    fn interval(&self) -> Duration {
        FRAME
    }
    /// The cells its picture covers, told each time it's laid out.
    fn cells(&mut self, _cols: u16, _rows: u16) {}
    /// The rows of words it keeps below the picture.
    fn words(&self) -> u16;
    /// Moves on by `dt` seconds and draws a picture `width` × `height`
    /// pixels.
    fn frame(&mut self, dt: f32, width: f32, height: f32) -> Frame;
    fn key(&mut self, _key: KeyCode) {}
    /// A click on the picture, in pixels from its center, y up.
    fn click(&mut self, _at: [f32; 2]) {}
    /// The glass's wash ([`flow_sdk::fx::tint`]) when it's changed since
    /// the last frame, to go out with this one in Flow; as the demo quits,
    /// the glass is washed off again.
    fn wash(&mut self) -> Option<String> {
        None
    }
}

/// What a demo in the screen's cells writes, and what goes after it: its
/// models' commands (Ratty's, which write cells of Flow's own, so they
/// follow the text that would cover them), the effects it sets off (OSC
/// 7731 escapes, which `--no-effects` leaves out), and Flow's sounds and
/// notes it plays (OSC 7731's `fx=sound` and DECPS), which go in Flow
/// either way.
#[derive(Debug, Default)]
pub struct Written {
    pub text: String,
    pub graphics: String,
    pub glass: Vec<String>,
    pub sounds: Vec<String>,
}

/// What a demo in cells needs the terminal to show besides text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Needs {
    Text,
    /// 3D models through the Ratty graphics protocol.
    Models,
}

/// A demo written in the screen's own cells, as text, with models of its
/// own placed among them where it needs them.
pub trait TextDemo {
    fn needs(&self) -> Needs {
        Needs::Text
    }
    fn told(&mut self, _told: Told) {}
    /// The least time between frames.
    fn interval(&self) -> Duration;
    /// Moves on by `dt` seconds and writes what changed on a screen of
    /// `size`: all of it the first time and after a resize.
    fn frame(&mut self, dt: f32, size: Size) -> Written;
    fn key(&mut self, _key: KeyCode) {}
    /// A click on a cell, counted from the screen's top left from 0.
    fn click(&mut self, _col: u16, _row: u16) {}
    /// What goes out as it quits: its models taken away, say.
    fn quit(&mut self) -> Written {
        Written::default()
    }
}

/// A demo, by how it draws.
pub enum Program {
    Staged(Box<dyn Demo>),
    Text(Box<dyn TextDemo>),
}

/// The demos, by the name that runs each, and what it is.
pub const DEMOS: [(&str, &str); 8] = [
    ("boing", "a checkered ball bouncing before a grid, after the Amiga's Boing demo of 1984"),
    ("cube", "a cube rolls out an F on a bigger one and fits into it: glass, metal, rock or jelly"),
    ("plasma", "a plasma made in code each frame and sent as a picture with kitty's protocol"),
    ("teletext", "pages of teletext, live: DEC's double lines, its mosaics and its eight colors"),
    ("vector", "glowing lines, as the arcades' vector monitors drew them: best under the CRT"),
    ("sounds", "a keyboard that plays Flow's notes (DECPS) and its sounds by name, with a tune"),
    ("tint", "a day passing over hills, the sky Flow's glass washed in the sky's colors"),
    ("model", "a balloon Flow reads from a file once you allow it, turning over the glass"),
];

pub fn demo(name: &str) -> Option<Program> {
    match name {
        "boing" => Some(Program::Staged(Box::new(boing::Boing::new()))),
        "cube" => Some(Program::Staged(Box::new(cube::Cube::new()))),
        "plasma" => Some(Program::Staged(Box::new(plasma::Plasma::new()))),
        "teletext" => Some(Program::Text(Box::new(teletext::Teletext::new()))),
        "vector" => Some(Program::Staged(Box::new(vector::Vector::new()))),
        "sounds" => Some(Program::Text(Box::new(sounds::Sounds::new()))),
        "tint" => Some(Program::Staged(Box::new(tint::Tint::new()))),
        "model" => Some(Program::Text(Box::new(balloon::Balloon::new()))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lines_lit_stretch_is_written_bold_in_its_own_color_between_the_rest() {
        let line = Line::new("glass · metal · rock", 0x808080).lit(8..13, 0x00ffff);
        assert_eq!(line.spans(), vec![("glass · ".to_string(), 0x808080, false), ("metal".to_string(), 0x00ffff, true), (" · rock".to_string(), 0x808080, false)]);
        assert_eq!(Line::new("rock", 0x808080).lit(0..4, 0xff8000).spans(), vec![("rock".to_string(), 0xff8000, true)], "nothing round it");
        assert_eq!(Line::new("q quits", 0x808080).bold().spans(), vec![("q quits".to_string(), 0x808080, true)], "a line lit nowhere is one span");
    }
}
