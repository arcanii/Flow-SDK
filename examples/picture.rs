//! Shows a picture below the prompt with the kitty graphics protocol: a
//! band of colors with a soft edge, sent once and shown by placeholder
//! cells, which then scroll with the text. In a terminal without kitty
//! pictures it says so instead.
//!
//!   cargo run -p flow-sdk --example picture

use std::io;

use flow_sdk::picture::{self, Ids, Image, PLACEMENT};
use flow_sdk::{term, Query};

fn main() -> io::Result<()> {
    // Raw while asking, so the answers aren't echoed.
    flow_sdk::crossterm::terminal::enable_raw_mode()?;
    let mut terminal = Query::new().pictures().cell().area().ask();
    flow_sdk::crossterm::terminal::disable_raw_mode()?;
    if !terminal.caps.pictures {
        println!("This terminal doesn't show kitty pictures, so here are some letters instead.");
        return Ok(());
    }

    // The picture's pixels fill its cells exactly.
    let (cols, rows) = (32u16, 6u16);
    let (cw, ch) = terminal.caps.cell_size(&term::size()?).unwrap_or((10.0, 20.0));
    let (w, h) = ((cols as f32 * cw).round() as u32, (rows as f32 * ch).round() as u32);
    let image = Image::rgba(w, h, band(w, h));
    let id = Ids::new(std::process::id()).next_id();

    if terminal.caps.bare {
        // Bare cells can't say their row, so each row is a picture of its own.
        for row in 0..rows {
            let strip = image.strip(row, rows).expect("raw pixels");
            terminal.send(picture::upload(id + u32::from(row), &strip, PLACEMENT, cols, 1).as_bytes())?;
        }
        for row in 0..rows {
            println!("{}", picture::bare_row(id + u32::from(row), cols));
        }
    } else {
        terminal.send(picture::upload(id, &image, PLACEMENT, cols, rows).as_bytes())?;
        for row in 0..rows {
            println!("{}", picture::placeholder_row(id, PLACEMENT, row, 0, cols));
        }
    }
    Ok(())
}

/// Hues across, fading out toward the top and bottom edges.
fn band(w: u32, h: u32) -> Vec<u8> {
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        let edge = 1.0 - ((y as f32 + 0.5) / h as f32 * 2.0 - 1.0).abs();
        let alpha = (edge * 3.0).min(1.0);
        for x in 0..w {
            let hue = x as f32 / w as f32 * 6.0;
            let c = |shift: f32| (((hue + shift) % 6.0 - 3.0).abs() - 1.0).clamp(0.0, 1.0);
            px.extend([c(0.0), c(4.0), c(2.0)].map(|v| (v * 255.0) as u8));
            px.push((alpha * 255.0) as u8);
        }
    }
    px
}
