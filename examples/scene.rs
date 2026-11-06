//! A cube turning in 3D over the whole window, drawn as a scene each frame
//! and sent with the Ratty graphics protocol; a click ripples Flow's liquid
//! where it lands, and q quits. It needs a terminal that shows Ratty
//! models: Flow on macOS or Linux.
//!
//!   cargo run -p flow-sdk --example scene

use std::io::{self, Write};
use std::time::{Duration, Instant};

use flow_sdk::fx::{Kind, Sound};
use flow_sdk::input::{self, Event};
use flow_sdk::scene::{add, apply, compose, mul, rgb, rotation, Camera, Scene, V3};
use flow_sdk::stage::{Fx, Stage};
use flow_sdk::{term, Query, Terminal};

fn main() -> io::Result<()> {
    term::leave_on_panic();
    term::enter(true)?;
    // Models only through the terminal: on Windows, Flow writes a model's
    // cells where ConPTY doesn't know of them.
    let mut terminal = Query::new().models().cell().without_pipe().ask();
    if !terminal.caps.models {
        term::leave();
        eprintln!("This terminal doesn't show 3D models through the Ratty graphics protocol.");
        return Ok(());
    }
    let result = run(&mut terminal);
    term::leave();
    result
}

fn run(terminal: &mut Terminal) -> io::Result<()> {
    let start = Instant::now();
    let mut stage: Option<Stage> = None;
    let mut fresh = true;
    let mut out = io::stdout();
    loop {
        let size = term::size()?;
        let mut bytes = Vec::new();
        if fresh {
            if let Some(old) = &stage {
                old.clear(&mut bytes);
            }
            bytes.extend(b"\x1b[2J");
            stage = Stage::new(size, terminal.caps.cell, 1).map(|s| s.made_by("flow-sdk scene example"));
            if let Some(s) = &stage {
                bytes.extend(format!("\x1b[{};1Hclick to ripple the liquid · q quits", s.below() + 1).as_bytes());
            }
            fresh = false;
        }
        if let Some(stage) = stage.as_mut() {
            stage.show(&cube(start.elapsed().as_secs_f32(), stage.width(), stage.height()), &mut bytes);
        }
        out.write_all(&bytes)?;
        out.flush()?;

        let due = Instant::now() + Duration::from_millis(33);
        let mut effects = String::new();
        while let Some(event) = input::next_before(due)? {
            match &event {
                Event::Key(key) if input::is_quit(key) => {
                    let mut bytes = Vec::new();
                    if let Some(stage) = &stage {
                        stage.clear(&mut bytes);
                    }
                    out.write_all(&bytes)?;
                    return out.flush();
                }
                Event::Resize(..) => fresh = true,
                _ => {}
            }
            if let (Some((col, row)), Some(stage)) = (input::left_click(&event), stage.as_ref()) {
                if let Some(at) = stage.point_at(col, row) {
                    effects.push_str(&stage.effect(&Fx::new(Kind::Ripple, at, 160.0, 0.6).sound(Sound::Drop)).escape());
                }
            }
        }
        if flow_sdk::in_flow() || terminal.caps.flow {
            terminal.send_effects(effects.as_bytes())?;
        }
    }
}

/// The cube at `t` seconds, in a picture `w` × `h` pixels.
fn cube(t: f32, w: f32, h: f32) -> Scene {
    let mut scene = Scene::new(w, h);
    let cam = Camera::orbit([0.0; 3], 0.0, 0.35, 6.0, 0.7, h);
    let turn = compose(rotation([0.0, 1.0, 0.0], t * 0.8), rotation([1.0, 0.0, 0.0], t * 0.5));
    let faces: [(V3, V3, V3, u32); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0], 0xe8231c),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0], 0x2a9df4),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0xf4f4f4),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0xf2c12e),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0x7ce7ff),
        ([0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0xb04ad0),
    ];
    for (n, u, v, color) in faces {
        let corner = |a: f32, b: f32| apply(turn, add(n, add(mul(u, a), mul(v, b))));
        let square = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)];
        cam.face(&mut scene, rgb(color), &square, apply(turn, n));
    }
    scene
}
